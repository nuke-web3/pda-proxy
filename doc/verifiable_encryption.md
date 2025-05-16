# Verifiable Encryption

### _A New Primitive Enabling Private Data Availability_

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
Applying VE with KMS keys allows anyone (person, smart contract, etc.) to verify that some critical data is available (although encrypted) _and_ the conditions required to access it.

## Use Cases

The authors have a few use cases described below, but we would **love** to hear what you might want to use VE and/or PDA for!
Please [open up an issue](https://github.com/celestiaorg/pda-proxy/issues) with feature requests and/or use case ideas!

### _Programmable Privacy Web3 dApps_

Strongly related to the exciting field of ["local-first access control"](https://www.inkandswitch.com/keyhive/notebook/), VE and PDA enable secure collaboration.
As chain data is shared to the entire network and easily accessible via indexers etc, we must use _encryption at rest_ to provide means of access control and selective disclosure.

#### Concrete Use Cases

- PDA as a Data Base for collaborative dApps with fine grained access control.
- Trustless sale of data via PDA and escrow contracts.
  See the [Stock0 hackathon project](https://dorahacks.io/buidl/14098) for inspiration!
- Private Rollups with **programmable cryptography** enabling [partial or fully obfuscated state](https://0xparc.org/blog/programmable-cryptography-1).
  With a [proxy service](../README.md), any exisitng DA user can easily move to PDA!

### _Verifiable Private Backups_

Critical data published publicly via PDA can be recovered _but only by predefined methods to decrypt it_.
Thus nothing is revealed about the data backed up, but anyone can audit that it exists and it is possible to recover it.

#### Concrete Use Cases

- Audits with sensitive data held with PDA such that it cannot be withheld and - although hidded from the public - can be recovered as needed.
- "Disaster" recovery where critical data is guaranteed to be obtainable in an encrypted form, with know methods to decrypt it defined.

## Example Architecture

The **anchor** provides a reference to **connect any protocol** via a **proof** that some **_private_ data was made available**.

![Verifiable Encryption Diagram](./assets/verifiable-encryption.drawio.svg)

## Future Work & Further Reading

Today there are limitations on the implentations of VE for PDA, but those should be relaxed significantly over time, enabling even more enticing uses for it!
Most notably there are hybrid scheme using a blend of TEE, MCP, and ZK Proof tooling that can bring about significant performance gains and confidential compute possibilities to allow for outsourcing and parallelization of PDA related tasks. 
We describe and expand on [research here](https://docs.google.com/document/d/1XZyuOxdMm5INcHwQZOZ8ALRk_YkvicNwQHSfOVs8hoM/)
