#!/bin/bash

DIR=$(dirname "$0")

VK_ENV="vulkan1.3"

glslc -O -fshader-stage=compute $DIR/src/shaders/shader.glsl --target-env=$VK_ENV -o $DIR/shaders/shader.spv
