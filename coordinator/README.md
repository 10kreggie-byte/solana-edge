# Coordinator

MVP responsibilities:

- create canonical deterministic jobs;
- assign jobs to workers;
- receive miner results;
- request independent verification;
- issue signed work receipts only after successful verification;
- batch receipts into reward epochs.

The MVP coordinator is intentionally centralized. Decentralizing scheduling comes after deterministic correctness and anti-replay accounting are proven.
