#!/bin/bash

DIR=$(dirname "$0")

VK_ENV="vulkan1.3"

glslc -O $DIR/src/shaders/shader.vert --target-env=$VK_ENV -o $DIR/shaders/vert.spv
glslc -O $DIR/src/shaders/shader.frag --target-env=$VK_ENV -o $DIR/shaders/frag.spv
