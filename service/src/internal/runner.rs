use crate::{Job, JobStatus, PdaRunnerError, SP1ProofSetup, SuccNetProgramId, internal::util};

use hex::FromHex;
use log::{debug, error, info, warn};
use sha2::Digest;
use sled::{Transactional, Tree as SledTree};
use sp1_sdk::{
    EnvProver as SP1EnvProver, SP1ProofWithPublicValues, SP1Stdin,
    network::Error as SP1NetworkError,
};
use std::sync::Arc;
use tokio::sync::{OnceCell, mpsc};

/// Hardcoded ELF binary for the crate `program-keccak-inclusion`
static CHACHA_ELF: &[u8] = include_bytes!(
    "../../../target/elf-compilation/riscv32im-succinct-zkvm-elf/release/chacha-program"
);
/// Hardcoded ID for the crate `program-keccak-inclusion`
static CHACHA_ID: OnceCell<SuccNetProgramId> = OnceCell::const_new();

/// Given a hard coded ELF, get it's ID
/// TODO: generalize
pub async fn get_program_id() -> SuccNetProgramId {
    *CHACHA_ID
        .get_or_init(|| async {
            debug!("Building Program ID");
            sha2::Sha256::digest(CHACHA_ELF).into()
        })
        .await
}

static CHACHA_SETUP: OnceCell<Arc<SP1ProofSetup>> = OnceCell::const_new();

/// TODO: setup ability to config as needed
pub struct PdaRunnerConfig {}

/// The main service runner.
pub struct PdaRunner {
    #[allow(dead_code)] // TODO: use config
    pub config: PdaRunnerConfig,
    pub config_db: SledTree,
    pub queue_db: SledTree,
    pub finished_db: SledTree,
    pub job_sender: mpsc::UnboundedSender<Option<Job>>,
    zk_client_handle: OnceCell<Arc<SP1EnvProver>>,
}

impl PdaRunner {
    /// As we need a [OnceCell] internal to this struct, you must use `new`
    /// to correctly construct it.
    pub fn new(
        config: PdaRunnerConfig,
        zk_client_handle: OnceCell<Arc<SP1EnvProver>>,
        config_db: SledTree,
        queue_db: SledTree,
        finished_db: SledTree,
        job_sender: mpsc::UnboundedSender<Option<Job>>,
    ) -> Self {
        PdaRunner {
            config,
            zk_client_handle,
            config_db,
            queue_db,
            finished_db,
            job_sender,
        }
    }

    /// The main method: sending a [Job] to this PDA Runner.
    /// If Job is finished, returns Ok(Some(Proof)),
    /// or Ok(None) if we don't have one (yet, likely pending)
    pub async fn get_verifiable_encryption(
        &self,
        job: Job,
    ) -> Result<Option<SP1ProofWithPublicValues>, PdaRunnerError> {
        let job_key = bincode::serialize(&job.anchor)
            .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?;
        info!("0x{} - Received request", hex::encode(&job_key));

        // Check DB for finished jobs
        if let Some(proof_data) = self
            .finished_db
            .get(&job_key)
            .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?
        {
            let job_status: JobStatus = bincode::deserialize(&proof_data)
                .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?;

            match job_status {
                JobStatus::ZkProofFinished(proof) => {
                    debug!("Job finished, returning proof from DB");
                    return Ok(Some(proof));
                }
                JobStatus::Failed(error, maybe_status) => {
                    match maybe_status {
                        None => {
                            warn!("Job is PERMANENT FAILURE, returning status");
                            return Err(PdaRunnerError::InternalError(format!("{error:?}")));
                        }
                        Some(retry_status) => {
                            warn!("Job is Retryable Failure, returning status & retrying");
                            // We retry errors on each call
                            // for a specific [Job] by sending to the queue
                            match self.send_job_with_new_status(job_key, *retry_status, job) {
                                Ok(_) => {
                                    return Err(PdaRunnerError::InternalError(format!(
                                        "Retrying! Previous error: {error:?}"
                                    )));
                                }
                                Err(e) => {
                                    return Err(PdaRunnerError::InternalError(format!(
                                        "PLEASE REPORT! Internal error, cannot retry: {e:?}"
                                    )));
                                }
                            }
                        }
                    }
                }
                _ => {
                    let e = "PLEASE REPORT! Finished DB is in invalid state";
                    error!("{e}");
                    return Err(PdaRunnerError::InternalError(e.to_string()));
                }
            }
        }

        // Check DB for pending jobs
        if let Some(queue_data) = self
            .queue_db
            .get(&job_key)
            .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?
        {
            debug!("Job in pending queue");
            let job_status: JobStatus = bincode::deserialize(&queue_data)
                .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?;
            match job_status {
                JobStatus::LocalZkProofPending => return Ok(None),
                _ => {
                    let e = format!(
                        "0x{} - Job queue is in invalid state",
                        hex::encode(&job_key)
                    );
                    error!("{e}");
                    return Err(PdaRunnerError::InternalError(e));
                }
            }
        }

        info!(
            "0x{} - New job sending to worker and adding to queue",
            hex::encode(&job_key)
        );
        self.queue_db
            .insert(
                &job_key,
                bincode::serialize(&JobStatus::LocalZkProofPending)
                    .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?,
            )
            .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?;

        self.job_sender
            .send(Some(job.clone()))
            .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?;

        Ok(None)
    }

    ///A worker that receives [Job]s by a channel and drives them to completion.
    ///
    /// `Job`s are complete stepwise with a [JobStatus] that is recorded in a
    /// queue database, with work to progress each status to towards completion async.
    /// Once a step is completed, `JobStatus` is recorded into the queue database that
    /// recursively, driving to `Job` completion.
    ///
    /// When a successful or failed state is arrived at,
    /// the job is atomically removed from the queue and added to a results database.
    pub async fn job_worker(
        self: Arc<Self>,
        mut job_receiver: mpsc::UnboundedReceiver<Option<Job>>,
    ) {
        debug!("Job worker started");
        while let Some(Some(job)) = job_receiver.recv().await {
            let service = self.clone();
            tokio::spawn(async move {
                debug!("Worker received job");
                let _ = service.prove(job).await; //Don't return with "?", we run keep looping
            });
        }

        info!("Shutting down");
        let _ = self.queue_db.flush();
        let _ = self.finished_db.flush();
        info!("Cleanup complete");

        std::process::exit(0);
    }

    /// The main service task: produce a proof based on a [Job] requested.
    async fn prove(&self, job: Job) -> Result<(), PdaRunnerError> {
        let job_key = bincode::serialize(&job.anchor)
            .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?;
        if let Some(queue_data) = self
            .queue_db
            .get(&job_key)
            .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?
        {
            let mut job_status: JobStatus = bincode::deserialize(&queue_data)
                .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?;
            debug!("Job worker processing with starting status: {job_status:?}");
            match job_status {
                JobStatus::LocalZkProofPending => {
                    // TODO handle non-hardcoded ZK programs
                    match self
                        .local_zk_proof(&get_program_id().await, &job, &job_key)
                        .await
                    {
                        Ok(zk_proof) => {
                            info!("0x{} - 🎉 Finished!", hex::encode(&job_key));
                            job_status = JobStatus::ZkProofFinished(zk_proof);
                            self.finalize_job(&job_key, job_status)?;
                        }
                        Err(e) => {
                            error!("0x{} - Failed progressing job: {e}", hex::encode(&job_key));
                            job_status = JobStatus::Failed(
                                e, None, // TODO: should this be retryable?
                            );
                            self.finalize_job(&job_key, job_status)?;
                        }
                    };
                    debug!("0x{} - ZKP stored in finalized DB", hex::encode(job_key));
                }
                _ => error!("Queue has INVALID status! Finished jobs stuck in queue!"),
            }
        };
        Ok(())
    }

    /// Given a SHA2 hash of a ZK program, get the require setup.
    /// The setup is a very heavy task and produces a large output (~200MB),
    /// fortunately it's identical per ZK program, so we store this in a DB to recall it.
    /// We load it and return a pointer to a single instance of this large setup object
    /// to read from for many concurrent [Job]s.
    pub async fn get_proof_setup(
        &self,
        zk_program_elf_sha2: &[u8; 32],
        zk_client_handle: Arc<SP1EnvProver>,
    ) -> Result<Arc<SP1ProofSetup>, PdaRunnerError> {
        debug!("Getting ZK program proof setup");
        let setup = CHACHA_SETUP
            .get_or_try_init(|| async {
                // Check DB for existing pre-computed setup
                let precomputed_proof_setup = self
                    .config_db
                    .get(zk_program_elf_sha2)
                    .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?;

                let proof_setup = match precomputed_proof_setup { Some(precomputed) => {
                    bincode::deserialize(&precomputed)
                        .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?
                } _ => {
                    info!(
                        "No ZK proof setup in DB for SHA2_256 = 0x{} -- generation & storing in config DB",
                        hex::encode(zk_program_elf_sha2)
                    );

                    let new_proof_setup: SP1ProofSetup = tokio::task::spawn_blocking(move || {
                        zk_client_handle.setup(CHACHA_ELF).into()
                    })
                    .await
                    .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?;

                    self.config_db
                        .insert(
                        zk_program_elf_sha2,
                        bincode::serialize(&new_proof_setup)
                            .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?,
                        )
                        .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?;

                    new_proof_setup
                }};
                Ok(Arc::new(proof_setup))
            })
            .await?
            .clone();

        Ok(setup)
    }

    /// Helper function to handle error from a SP1 NetworkProver Clients.
    /// Will finalize the job in an [JobStatus::Failed] state,
    /// that may be retryable.
    fn handle_zk_client_error(
        &self,
        zk_client_error: &SP1NetworkError,
        job_key: &[u8],
    ) -> PdaRunnerError {
        error!("SP1 Client error: {zk_client_error}");
        let (e, job_status);
        match zk_client_error {
            SP1NetworkError::SimulationFailed | SP1NetworkError::RequestUnexecutable { .. } => {
                e = PdaRunnerError::DaClientError(format!(
                    "ZKP program critical failure: {zk_client_error} occurred for job 0x{} PLEASE REPORT!",
                    hex::encode(job_key)
                ));
                job_status = JobStatus::Failed(e.clone(), None);
            }
            SP1NetworkError::RequestUnfulfillable { .. } => {
                e = PdaRunnerError::DaClientError(format!(
                    "ZKP network failure: {zk_client_error} occurred for job 0x{} PLEASE REPORT!",
                    hex::encode(job_key)
                ));
                job_status = JobStatus::Failed(e.clone(), None);
            }
            SP1NetworkError::RequestTimedOut { request_id } => {
                e = PdaRunnerError::DaClientError(format!(
                    "ZKP network: {zk_client_error} occurred for job 0x{}",
                    hex::encode(job_key)
                ));

                let id = request_id
                    .as_slice()
                    .try_into()
                    .expect("request ID is always correct length");
                job_status =
                    JobStatus::Failed(e.clone(), Some(JobStatus::RemoteZkProofPending(id).into()));
            }
            SP1NetworkError::RpcError(_) | SP1NetworkError::Other(_) => {
                e = PdaRunnerError::DaClientError(format!(
                    "ZKP network failure: {zk_client_error} occurred for job 0x{} PLEASE REPORT!",
                    hex::encode(job_key)
                ));
                // TODO: We cannot clone thus we cannot insert into a JobStatus::...(proof_input)
                // So we just redo the work from scratch for the DA side as a stupid workaround
                job_status = JobStatus::Failed(e.clone(), None);
            }
        }
        match self.finalize_job(job_key, job_status) {
            Ok(_) => e,
            Err(internal_err) => internal_err,
        }
    }

    async fn local_zk_proof(
        &self,
        program_id: &SuccNetProgramId,
        job: &Job,
        job_key: &[u8],
    ) -> Result<SP1ProofWithPublicValues, PdaRunnerError> {
        debug!("Preparing local SP1 proving");
        let zk_client_handle = self.get_zk_client().await;
        let proof_setup = self
            .get_proof_setup(program_id, zk_client_handle.clone())
            .await?;

        let mut stdin = SP1Stdin::new();
        // Setup the inputs:
        // - key = 32 bytes
        // - nonce = 12 bytes (MUST BE UNIQUE - NO REUSE!)
        // - input_plaintext = bytes to encrypt

        let key = <[u8; 32]>::from_hex(
            std::env::var("ENCRYPTION_KEY").expect("Missing ENCRYPTION_KEY env var"),
        )
        .expect("ENCRYPTION_KEY must be 32 bytes, hex encoded (ex: `1234...abcd`)");

        stdin.write_slice(&key);

        let nonce: [u8; 12] = util::random_nonce();
        stdin.write_slice(&nonce);

        // TODO: replace example bytes with service interface
        let input_plaintext: &[u8] = job.input.data.as_slice();
        stdin.write_slice(input_plaintext);

        debug!("0x{} - Starting proof", hex::encode(job_key));
        let proof = zk_client_handle
            .prove(&proof_setup.pk, &stdin)
            .groth16()
            .run()
            // TODO: how to handle errors without a concrete type? Anyhow is not the right thing for us...
            .map_err(|e| {
                if let Some(down) = e.downcast_ref::<SP1NetworkError>() {
                    return self.handle_zk_client_error(down, job_key);
                }
                PdaRunnerError::ZkClientError(format!("Unhandled Error: {e} PLEASE REPORT"))
            })?;
        debug!("0x{} - Proof complete", hex::encode(job_key));
        Ok(proof)
    }

    /// Atomically move a job from the database queue tree to the proof tree.
    /// This removes the job from any further processing by workers.
    /// The [JobStatus] should be success or failure only
    /// (but this is not enforced or checked at this time)
    fn finalize_job(&self, job_key: &[u8], job_status: JobStatus) -> Result<(), PdaRunnerError> {
        // TODO: do we want to do a status check here? To prevent accidentally getting into a DB invalid state
        (&self.queue_db, &self.finished_db)
            .transaction(|(queue_tx, finished_tx)| {
                queue_tx.remove(job_key)?;
                finished_tx.insert(
                    job_key,
                    bincode::serialize(&job_status).expect("Always given serializable job status"),
                )?;
                Ok::<(), sled::transaction::ConflictableTransactionError<PdaRunnerError>>(())
            })
            .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?;
        Ok(())
    }

    /// Insert a [JobStatus] into a [SledTree] database
    /// AND `send()` this job back to the `self.job_sender` to schedule more progress.
    /// You likely want to pass `self.some_sled_tree` into `data_base` as input.
    fn send_job_with_new_status(
        &self,
        job_key: Vec<u8>,
        update_status: JobStatus,
        job: Job,
    ) -> Result<(), PdaRunnerError> {
        debug!(
            "Sending job 0x{} back with updated status: {update_status:?}",
            hex::encode(&job_key)
        );
        (&self.queue_db, &self.finished_db)
            .transaction(|(queue_tx, finished_tx)| {
                finished_tx.remove(job_key.clone())?;
                queue_tx.insert(
                    job_key.clone(),
                    bincode::serialize(&update_status)
                        .expect("Always given serializable job status"),
                )?;
                Ok::<(), sled::transaction::ConflictableTransactionError<PdaRunnerError>>(())
            })
            .map_err(|e| PdaRunnerError::InternalError(e.to_string()))?;
        self.job_sender
            .send(Some(job))
            .map_err(|e| PdaRunnerError::InternalError(e.to_string()))
    }

    pub async fn get_zk_client(&self) -> Arc<SP1EnvProver> {
        self.zk_client_handle
            .get_or_init(|| async {
                debug!("Building ZK client");
                let client = sp1_sdk::ProverClient::from_env();
                Arc::new(client)
            })
            .await
            .clone()
    }

    pub fn shutdown(&self) {
        info!("Terminating worker,finishing preexisting jobs");
        let _ = self.job_sender.send(None); // Break loop in `job_worker`
    }
}
