
EinsteinDB - The world's first open-source, distributed, relativistic serializable, Byzantine Fault-Tolerant, Transactional APIs with traditional SQL Hybrid OLAP/OLTP for HTAP workloads. 

Built with Crossbeam, Tokio, and a re-designed epoch mechanism: EinsteinDB provides a unified strategy for storing new data and moving it between devices, across the spectrum of time (a la postgres), spearheading a proliferation of stores and strategies within a robust mechanism for syncing subsets of data collected (guaranteeing low-latency and partition fault-tolerance). EinsteinDB is borne out of the needs of modern-day cloud-native infrastructure: portable, performant, persistent, and embedded. ACID complaint.

The central approach employed in EinsteinDB is tracking and explicitly checking whether causal dependencies between keys are satisfied in the local cluster before exposing writes. Further, in EinsteinDB, we introduce get transactions in order to obtain a consistent view of multiple keys without locking or blocking. EinsteinDB completes operations in less than a millisecond, provides throughput similar to previous systems when using a single cluster, and scales well as we increase the number of servers in each cluster.

EinsteinDB allows read and write operations to continue even during network partitions and resolves updated conflicts using stateless hash trees. It uses an append-log to preserve data integrity, replicates each hyperlog on multiple servers for durability via BFT Raft consensus implemented via Haskell (Rust Compiler) to render highly-efficient algebraic queries and machine-code interpretation of instruction pipelines maintaining sparse, multi-dimensional sorted maps and allows applications to access their data using a partial order DAG.

