Coordinator 2PC, the experimental tool to synchronize distributed transactions guided by the two-phase commit approach.

Currently it works only over HTTP.

### How does it work? (optimistic scenario)

0. Coordinator gets request with messages and distributes them to services.
1. Each service handles received message, prepares transaction and sends confirmation request to coordinator.
2. When coordinator gets confirmation requests from all expected services, it respond them that transactions can be commited.
3. Services commit transations.

### Process sequence diagram

![Process sequence diagram](https://github.com/vTinMan/coordinator_2pc/blob/main/process_diagram.png?raw=true)

### Note

Two-phase commit approach does not guarantee entire consistency, it only may speed up service synchronization and decrease number of rollbacks.
