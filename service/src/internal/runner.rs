use crate::internal::error::PdaServiceError;
use crate::{Job, JobStatus, SP1ProofSetup, SuccNetProgramId};

use jsonrpsee::core::ClientError as JsonRpcError;
use log::{debug, error, info};
use sha2::Digest;
use sled::{Transactional, Tree as SledTree};
use sp1_sdk::{
    EnvProver as SP1EnvProver, SP1ProofWithPublicValues, SP1Stdin,
    network::Error as SP1NetworkError,
};
use std::sync::Arc;
use tokio::sync::{OnceCell, mpsc};

use super::job::Input;

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

/// The main service runner.
pub struct PdaRunner {
    pub config: PdaServiceConfig,
    pub config_db: SledTree,
    pub queue_db: SledTree,
    pub finished_db: SledTree,
    pub job_sender: mpsc::UnboundedSender<Option<Job>>,
    zk_client_handle: OnceCell<Arc<SP1EnvProver>>,
}

impl PdaRunner {
    pub fn new(
        config: PdaServiceConfig,
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
}

pub struct PdaServiceConfig {
    pub da_node_token: String,
    pub da_node_ws: String,
}

impl PdaRunner {
    /// A worker that receives [Job]s by a channel and drives them to completion.
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
                debug!("Job worker received {job:?}",);
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
    pub async fn prove(&self, job: Job) -> Result<(), PdaServiceError> {
        let job_key = bincode::serialize(&job.anchor)
            .map_err(|e| PdaServiceError::InternalError(e.to_string()))?;
        if let Some(queue_data) = self
            .queue_db
            .get(&job_key)
            .map_err(|e| PdaServiceError::InternalError(e.to_string()))?
        {
            let mut job_status: JobStatus = bincode::deserialize(&queue_data)
                .map_err(|e| PdaServiceError::InternalError(e.to_string()))?;
            debug!("Job worker processing with starting status: {job_status:?}");
            match job_status {
                JobStatus::LocalZkProofPending(proof_input) => {
                    // TODO handle non-hardcoded ZK programs
                    match self
                        .local_zk_proof(&get_program_id().await, &proof_input, &job, &job_key)
                        .await
                    {
                        Ok(zk_proof) => {
                            info!("🎉 {job:?} Finished!");
                            job_status = JobStatus::ZkProofFinished(zk_proof);
                            self.finalize_job(&job_key, job_status)?;
                        }
                        Err(e) => {
                            error!("{job:?} failed progressing DataAvailable: {e}");
                            job_status = JobStatus::Failed(
                                e, None, // TODO: should this be retryable?
                            );
                            self.finalize_job(&job_key, job_status)?;
                        }
                    };
                    debug!("ZK request sent");
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
    ) -> Result<Arc<SP1ProofSetup>, PdaServiceError> {
        debug!("Getting ZK program proof setup");
        let setup = CHACHA_SETUP
            .get_or_try_init(|| async {
                // Check DB for existing pre-computed setup
                let precomputed_proof_setup = self
                    .config_db
                    .get(zk_program_elf_sha2)
                    .map_err(|e| PdaServiceError::InternalError(e.to_string()))?;

                let proof_setup = match precomputed_proof_setup { Some(precomputed) => {
                    bincode::deserialize(&precomputed)
                        .map_err(|e| PdaServiceError::InternalError(e.to_string()))?
                } _ => {
                    info!(
                        "No ZK proof setup in DB for SHA2_256 = 0x{} -- generation & storing in config DB",
                        hex::encode(zk_program_elf_sha2)
                    );

                    let new_proof_setup: SP1ProofSetup = tokio::task::spawn_blocking(move || {
                        zk_client_handle.setup(CHACHA_ELF).into()
                    })
                    .await
                    .map_err(|e| PdaServiceError::InternalError(e.to_string()))?;

                    self.config_db
                        .insert(
                        zk_program_elf_sha2,
                        bincode::serialize(&new_proof_setup)
                            .map_err(|e| PdaServiceError::InternalError(e.to_string()))?,
                        )
                        .map_err(|e| PdaServiceError::InternalError(e.to_string()))?;

                    new_proof_setup
                }};
                Ok(Arc::new(proof_setup))
            })
            .await?
            .clone();

        Ok(setup)
    }

    /// Helper function to handle error from a [jsonrpsee] based DA client.
    /// Will finalize the job in an [JobStatus::Failed] state,
    /// that may be retryable.
    fn handle_da_client_error(
        &self,
        da_client_error: JsonRpcError,
        job: &Job,
        job_key: &[u8],
    ) -> PdaServiceError {
        error!("Celestia Client error: {da_client_error}");
        let (e, job_status);
        let call_err = "DA Call Error: ".to_string() + &da_client_error.to_string();
        match da_client_error {
            JsonRpcError::Call(error_object) => {
                // TODO: make this handle errors much better! JSON stringiness is a problem!
                if error_object.message().starts_with("header: not found") {
                    e = PdaServiceError::DaClientError(format!(
                        "{call_err} - Likely DA Node is not properly synced, and blob does exists on the network. PLEASE REPORT!"
                    ));
                    job_status = JobStatus::Failed(e.clone(), Some(JobStatus::Prerun.into()));
                } else if error_object
                    .message()
                    .starts_with("header: given height is from the future")
                {
                    e = PdaServiceError::DaClientError(call_err.to_string());
                    job_status = JobStatus::Failed(e.clone(), None);
                } else if error_object
                    .message()
                    .starts_with("header: syncing in progress")
                {
                    e = PdaServiceError::DaClientError(format!(
                        "{call_err} - Blob *may* exist on the network."
                    ));
                    job_status = JobStatus::Failed(e.clone(), Some(JobStatus::Prerun.into()));
                } else if error_object.message().starts_with("blob: not found") {
                    e = PdaServiceError::DaClientError(format!(
                        "{call_err} - Likely incorrect request inputs."
                    ));
                    job_status = JobStatus::Failed(e.clone(), None);
                } else {
                    e = PdaServiceError::DaClientError(format!(
                        "{call_err} - UNKNOWN DA client error. PLEASE REPORT!"
                    ));
                    job_status = JobStatus::Failed(e.clone(), None);
                }
            }
            JsonRpcError::RequestTimeout
            | JsonRpcError::Transport(_)
            | JsonRpcError::RestartNeeded(_) => {
                e = PdaServiceError::DaClientError(format!("{da_client_error}"));
                job_status = JobStatus::Failed(e.clone(), Some(JobStatus::Prerun.into()));
            }
            // TODO: handle other Celestia JSON RPC errors
            _ => {
                e = PdaServiceError::DaClientError(
                    "Unhandled Celestia SDK error. PLEASE REPORT!".to_string(),
                );
                error!("{job:?} failed, not recoverable: {e}");
                job_status = JobStatus::Failed(e.clone(), None);
            }
        };
        match self.finalize_job(job_key, job_status) {
            Ok(_) => e,
            Err(internal_err) => internal_err,
        }
    }

    /// Helper function to handle error from a SP1 NetworkProver Clients.
    /// Will finalize the job in an [JobStatus::Failed] state,
    /// that may be retryable.
    fn handle_zk_client_error(
        &self,
        zk_client_error: &SP1NetworkError,
        job: &Job,
        job_key: &[u8],
    ) -> PdaServiceError {
        error!("SP1 Client error: {zk_client_error}");
        let (e, job_status);
        match zk_client_error {
            SP1NetworkError::SimulationFailed | SP1NetworkError::RequestUnexecutable { .. } => {
                e = PdaServiceError::DaClientError(format!(
                    "ZKP program critical failure: {zk_client_error} occurred for {job:?} PLEASE REPORT!"
                ));
                job_status = JobStatus::Failed(e.clone(), None);
            }
            SP1NetworkError::RequestUnfulfillable { .. } => {
                e = PdaServiceError::DaClientError(format!(
                    "ZKP network failure: {zk_client_error} occurred for {job:?} PLEASE REPORT!"
                ));
                job_status = JobStatus::Failed(e.clone(), None);
            }
            SP1NetworkError::RequestTimedOut { request_id } => {
                e = PdaServiceError::DaClientError(format!(
                    "ZKP network: {zk_client_error} occurred for {job:?}"
                ));

                let id = request_id
                    .as_slice()
                    .try_into()
                    .expect("request ID is always correct length");
                job_status =
                    JobStatus::Failed(e.clone(), Some(JobStatus::RemoteZkProofPending(id).into()));
            }
            SP1NetworkError::RpcError(_) | SP1NetworkError::Other(_) => {
                e = PdaServiceError::DaClientError(format!(
                    "ZKP network failure: {zk_client_error} occurred for {job:?} PLEASE REPORT!"
                ));
                // TODO: We cannot clone thus we cannot insert into a JobStatus::...(proof_input)
                // So we just redo the work from scratch for the DA side as a stupid workaround
                job_status = JobStatus::Failed(e.clone(), Some(JobStatus::Prerun.into()));
            }
        }
        match self.finalize_job(job_key, job_status) {
            Ok(_) => e,
            Err(internal_err) => internal_err,
        }
    }

    pub async fn local_zk_proof(
        &self,
        program_id: &SuccNetProgramId,
        proof_input: &Input,
        job: &Job,
        job_key: &[u8],
    ) -> Result<SP1ProofWithPublicValues, PdaServiceError> {
        debug!("Preparing prover network request and starting proving");
        let zk_client_handle = self.get_zk_client().await;
        let proof_setup = self
            .get_proof_setup(program_id, zk_client_handle.clone())
            .await?;

        let mut stdin = SP1Stdin::new();
        stdin.write(&proof_input);
        let proof = zk_client_handle
            .prove(&proof_setup.pk, &stdin)
            .groth16()
            .run()
            // TODO: how to handle errors without a concrete type? Anyhow is not the right thing for us...
            .map_err(|e| {
                if let Some(down) = e.downcast_ref::<SP1NetworkError>() {
                    return self.handle_zk_client_error(down, job, job_key);
                }
                PdaServiceError::ZkClientError(format!("Unhandled Error: {e} PLEASE REPORT"))
            })?;

        Ok(proof)
    }

    /// Atomically move a job from the database queue tree to the proof tree.
    /// This removes the job from any further processing by workers.
    /// The [JobStatus] should be success or failure only
    /// (but this is not enforced or checked at this time)
    fn finalize_job(&self, job_key: &[u8], job_status: JobStatus) -> Result<(), PdaServiceError> {
        // TODO: do we want to do a status check here? To prevent accidentally getting into a DB invalid state
        (&self.queue_db, &self.finished_db)
            .transaction(|(queue_tx, finished_tx)| {
                queue_tx.remove(job_key)?;
                finished_tx.insert(
                    job_key,
                    bincode::serialize(&job_status).expect("Always given serializable job status"),
                )?;
                Ok::<(), sled::transaction::ConflictableTransactionError<PdaServiceError>>(())
            })
            .map_err(|e| PdaServiceError::InternalError(e.to_string()))?;
        Ok(())
    }

    /// Insert a [JobStatus] into a [SledTree] database
    /// AND `send()` this job back to the `self.job_sender` to schedule more progress.
    /// You likely want to pass `self.some_sled_tree` into `data_base` as input.
    pub fn send_job_with_new_status(
        &self,
        job_key: Vec<u8>,
        update_status: JobStatus,
        job: Job,
    ) -> Result<(), PdaServiceError> {
        debug!("Sending {job:?} back with updated status: {update_status:?}");
        (&self.queue_db, &self.finished_db)
            .transaction(|(queue_tx, finished_tx)| {
                finished_tx.remove(job_key.clone())?;
                queue_tx.insert(
                    job_key.clone(),
                    bincode::serialize(&update_status)
                        .expect("Always given serializable job status"),
                )?;
                Ok::<(), sled::transaction::ConflictableTransactionError<PdaServiceError>>(())
            })
            .map_err(|e| PdaServiceError::InternalError(e.to_string()))?;
        self.job_sender
            .send(Some(job))
            .map_err(|e| PdaServiceError::InternalError(e.to_string()))
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
