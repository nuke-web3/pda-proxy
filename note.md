# changes from eq-service

- moved to sha2 as it's more performant in zkVM and don't want to deal w/ mixed hashing
- custom error
- removed need for Celesita client calls internally
- simpler state machine
- use `let client = sp1_sdk::ProverClient::from_env();` instead of had coded network one.


- Ask for `Any` to downcast `EnvProver` <https://chatgpt.com/c/67feb651-100c-8001-b595-f55ddf260c2c>
  - <https://doc.rust-lang.org/std/any/index.html>


## Security

- There is no auth, so anyone can cause a zk proof to be started, and this blocks all other Jobs from progressing  
- Should likely not allow more than one job on the queue at any time? or allow to stack?
