# Private DA Middleware Proxy

> ⚠
> ***NOTICE***
> Work in Progress!
> NOT ready for use!
> ⚠

A [Celestia](https://celestia.org) client for the [`Blob` and `State` JSON RPC namespaces](https://node-rpc-docs.celestia.org/) enabling sensitive data to be encrypted before submission on the (public) network, and enable decryption on retrieval.
Non-sensitive calls are unmodified.

- [ ] `blob.Submit` request encrypts before sending
- [ ] `blob.Get` request decrypts before responding
- [ ] `state.Balance` & `state.AccountAddress` & `state.Transfer` proxy
- [ ] Fully compliant proxy for non-sensitive data `Blob` and `State` API

## Known Limitations

At time of writing, as it should be possible to change these limitations:

- Assumes that there is a single blob per transaction, no logic to handle multiple blobs.

Possible to change these, but requires upstream involvment:

- [Max blob size on Celestia](https://docs.celestia.org/how-to-guides/submit-data#maximum-blob-size) is presently ~2MB
- Upstream jsonrpsee en/decryption middleware feature into lumina?

## Acknowledgments

Based heavily on:
- https://github.com/paritytech/jsonrpsee/blob/master/examples/examples/proc_macro.rs
- https://github.com/eigerco/lumina/tree/main/rpc
