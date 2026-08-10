---
id: ieee-conference
source_format: ieee-html
title: "Real-Time Object Detection on FPGA Accelerators"
authors:
  - "Takahashi, Yuto"
doi: "10.1109/CVPR.2024.00456"
journal: "IEEE/CVF Conference on Computer Vision and Pattern Recognition"
---

# Real-Time Object Detection on FPGA Accelerators

## Abstract

We demonstrate **real-time object detection** at 120 fps on a low-power FPGA using quantized neural networks.

## Architecture

The detector uses a *single-shot* architecture with depthwise separable convolutions, mapped to fixed-point arithmetic on the FPGA fabric.

### Quantization Strategy

Weights are quantized to 4 bits and activations to 8 bits, achieving 6.2× compression over FP32.

## Results

Performance comparison on COCO val2017:

| Model | mAP | FPS | Power (W) |
| --- | --- | --- | --- |
| YOLOv8-n (GPU) | 37.3 | 450 | 75 |
| YOLOv8-n (FPGA) | 35.1 | 120 | 8.2 |
| Ours (FPGA) | 36.8 | 120 | 5.7 |

