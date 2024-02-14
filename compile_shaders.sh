#!/bin/bash

DIR=$(dirname "$0")

VK_ENV="vulkan1.3"

glslc -O $DIR/src/render/shaders/player/shader.vert --target-env=$VK_ENV -o $DIR/shaders/player/vert.spv
glslc -O $DIR/src/render/shaders/player/shader.frag --target-env=$VK_ENV -o $DIR/shaders/player/frag.spv

glslc -O $DIR/src/render/shaders/projectiles/shader.vert --target-env=$VK_ENV -o $DIR/shaders/projectiles/vert.spv
glslc -O $DIR/src/render/shaders/projectiles/shader.frag --target-env=$VK_ENV -o $DIR/shaders/projectiles/frag.spv
