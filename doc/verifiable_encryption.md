# Verifiable Encryption<br>_A New Tool Enabling Private Data Availability_

> "Don't trust. Verify."

In this document, we introduce Verifiable Encryption (VE) schema and how they enable Private Data Availability (PDA).

## Verifiable Encryption

Encrypted data (ciphertext) should reveal **_nothing_** about the encrypted data (plaintext) it was created from, thus you _cannot verify anything about the plaintext_ without decrypting it.
Therefore, one must _fully trust those that can decrypt_ about anything the plaintext relates to.
We want to _constrain_ the plaintext, key(s) used, encryption algorithm, etc. when generating ciphertext such that _the public can verify_ that it is decrypt-able to some plaintext with specific properties _without the ability to decrypt it_.

## Data Availability

When data is required for a protocol's safety and/or functionality, we demand robust guarantees on _public_ Data Availability (DA).
So even in hostile conditions trying to stop others from accessing critical data, a protocol can continue to operate.
But in many cases, that data may be _too sensitive_ for the whole world to know.
We must find a way to _selectively disclose specific aspects_ only to _prearranged parties_ under specific conditions.

## With our powers combined...

PDA becomes a powerful new primitive that is highly configurable with VE.
Integrating with any existing or novel Key Management Systems (KMS), one is able to define the conditions that surrounding encryption key recovery and distribution.
Applying VE with KMS keys allows anyone (person, smart contract, etc.) to verify that some critical data is available (although encrypted) _and_ the coitions required to access it.

## Example Architecture

The **anchor** provides a reference to **connect any protocol** via a **proof** that some **_private_ data was made available**.

![Verifiable Encryption Diagram](./assets/verifiable-encryption.drawio.svg)

## Present Limitations

For PDA Proxy impl in this repo:

TODO: expand on bullets:

- zkVM performance & bottleneck
- prover sees sensitive inputs (Need confidential compute)
  - cannot outsource (PaaS) for now
-
-

## Future Work & Further Reading

TODO: describe and expand on [research here](https://docs.google.com/document/d/1XZyuOxdMm5INcHwQZOZ8ALRk_YkvicNwQHSfOVs8hoM/)
