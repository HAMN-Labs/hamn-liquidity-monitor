# HAMN: Hierarchical Adaptive Memory Network for Liquidity Analysis

**HAMN** is an intelligent system designed to analyze and forecast liquidity flows within blockchain ecosystems such as Ethereum and Arbitrum. The system operates in a streaming mode and autonomously adapts to market shifts without the need for manual rule updates.

## Key Features
*   **Real-Time Liquidity Detection:** The system identifies token swaps, liquidity additions and removals, arbitrage sequences, and capital migrations.
*   **ABI-Agnostic Recognition:** HAMN recognizes new pools and protocols based on behavioral features—such as token movements, reserve changes, and Mint/Burn operations—rather than relying on static contract addresses or ABIs.
*   **Predictive Analytics:** By constructing probabilistic **transition maps**, the system forecasts likely subsequent actions of market participants based on accumulated pattern memory.
*   **Bot and Whale Monitoring:** The architecture tracks MEV bot strategies and large-scale participant behavior by analyzing transaction complexity and specific asset movement patterns.

## Technical Architecture
The HAMN architecture implements a full real-time data processing cycle:
1.  **Data Ingestion:** Fetching blocks, transactions, and event logs from the network.
2.  **Feature Extraction:** Calculating behavioral feature vectors, including reserve shifts and transaction complexity.
3.  **Adaptive Memory:** Matching vectors against existing patterns, creating new entries, and automatically pruning obsolete data to ensure stability.
4.  **Sequence Mapping:** Establishing action sequences and calculating transition probabilities for the predictive model.
5.  **Online Processing:** High-throughput streaming with low latency, optimized for fast-block networks like **Arbitrum**.

## Implementation Roadmap (MVP)
The development is structured into six key stages:
*   **Stage 1:** Implementing block and transaction fetching mechanisms.
*   **Stage 2:** Building the behavioral feature extraction engine.
*   **Stage 3:** Developing the pattern memory and vector matching logic.
*   **Stage 4:** Integrating memory stabilization and cleanup algorithms.
*   **Stage 5:** Training the sequence model and constructing the transition map.
*   **Stage 6:** Launching full-scale online detection of liquidity events.

## Success Metrics
The MVP is considered successful upon meeting the following criteria:
*   Correct detection of swaps and liquidity changes.
*   Stable memory performance over extended periods.
*   Accurate discovery of new pools.

## Use Cases
*   Monitoring DeFi activity and pool liquidity fluctuations.
*   Automatically discovering new protocols and pools immediately upon deployment.
*   Generating trading signals based on probabilistic capital movement forecasts.
*   Researching market patterns and MEV bot behaviors.

---
*This project is implemented in accordance with technical specification version 0.2.*
