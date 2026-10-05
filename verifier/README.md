# Verifier

The verifier independently executes the exact same canonical job specification as the miner.

MVP acceptance rule:

`miner.commitment == verifier.commitment`

A mismatch rejects the work receipt and should preserve both outputs for audit/debugging.

Later versions will add redundant verifiers, random audits, reputation, challenge windows, and threshold authorization.
