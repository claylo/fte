---
id: ieee-many-authors
source_format: ieee-html
title: "A Survey of Large Language Models for Code Generation"
authors:
  - "Chen, Mark"
  - "Tworek, Jerry"
  - "Jun, Heewoo"
  - "Yuan, Qiming"
  - "Pinto, Henrique"
  - "Kaplan, Jared"
  - "Edwards, Harrison"
  - "Burda, Yuri"
doi: "10.1109/TSE.2024.3456789"
journal: "IEEE Transactions on Software Engineering"
---

# A Survey of Large Language Models for Code Generation

## Abstract

We survey **72 large language models** for code generation, evaluating performance across six programming languages on the HumanEval and MBPP benchmarks.

## Introduction

Code generation using neural language models has advanced rapidly since the introduction of *Codex* and its successors.

## Taxonomy of Models

We categorize models along three axes:

1. Architecture: encoder-decoder vs decoder-only
2. Training data: natural language + code vs code-only
3. Fine-tuning: instruction-tuned vs base models

## Benchmark Results

| Model | Params (B) | HumanEval | MBPP |
| --- | --- | --- | --- |
| GPT-4 | — | 67.0 | — |
| Claude 3.5 | — | 64.0 | — |
| CodeLlama | 34 | 53.7 | 56.2 |
| StarCoder2 | 15 | 46.3 | 52.8 |
| DeepSeek-Coder | 33 | 56.1 | 58.4 |

## Discussion

Model scale alone does not determine performance; **training data quality** and instruction tuning have outsized effects on benchmark scores.

