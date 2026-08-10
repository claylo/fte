---
id: elsevier-article
source_format: elsevier-html
title: "Deep Reinforcement Learning for Autonomous Navigation"
authors:
  - "Müller, Hans"
  - "Kapoor, Ananya"
doi: "10.1016/j.robot.2024.104567"
journal: "Robotics and Autonomous Systems"
---

# Deep Reinforcement Learning for Autonomous Navigation

## Abstract

We present a **deep reinforcement learning** framework for autonomous robot navigation in unstructured environments.

## Introduction

Autonomous navigation requires agents to make sequential decisions under uncertainty. Traditional approaches rely on *simultaneous localization and mapping* (SLAM), but learned policies can adapt to novel environments.

## Method

### State Representation

The agent receives a 64×64 depth image and its current velocity vector as input.

### Reward Design

The reward function balances three objectives:

2. Progress toward the goal (+1.0 per meter)
4. Collision avoidance (−10.0 per contact)
6. Smoothness penalty (−0.1 per angular velocity change)

## Experiments

We evaluated in both simulated and real-world environments:

| Environment | Success Rate (%) | Avg. Time (s) |
| --- | --- | --- |
| Indoor (sim) | 94.2 | 12.3 |
| Outdoor (sim) | 87.6 | 28.7 |
| Indoor (real) | 88.1 | 15.8 |

## Conclusion

Our approach achieves **near-human performance** in cluttered environments while maintaining real-time inference speeds.

