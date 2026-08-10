---
id: ieee-article
source_format: ieee-html
title: "Energy-Efficient Federated Learning for IoT Edge Devices"
authors:
  - "Kim, Hyunwoo"
  - "Ramirez, Carlos"
doi: "10.1109/TPAMI.2024.3401234"
journal: "IEEE Transactions on Pattern Analysis and Machine Intelligence"
---

# Energy-Efficient Federated Learning for IoT Edge Devices

## Abstract

We propose an **energy-aware** federated learning protocol that reduces communication costs by 73% while maintaining model accuracy.

## Introduction

Federated learning enables collaborative model training without sharing raw data. However, the *communication overhead* between edge devices and the aggregation server remains a bottleneck for resource-constrained IoT deployments.

## Proposed Method

### Gradient Compression

We apply top-k sparsification with adaptive threshold selection based on device battery level.

### Asynchronous Aggregation

Devices contribute updates at heterogeneous intervals without blocking the global model.

## Experimental Results

| Method | Accuracy (%) | Comm. (MB) | Energy (J) |
| --- | --- | --- | --- |
| FedAvg | 91.2 | 847 | 42.3 |
| FedProx | 90.8 | 847 | 41.9 |
| Ours | 90.5 | 229 | 11.4 |

## Conclusion

Our approach enables **practical federated learning** on battery-powered devices with minimal accuracy loss.

