#!/bin/sh

docker buildx build --platform linux/amd64 --push -t ghcr.io/skearya/skearything .
