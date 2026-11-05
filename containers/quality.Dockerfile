# syntax=docker/dockerfile:1.7
ARG BASE_IMAGE=ghcr.io/cpeter1207/rpt-advanced-quality-debian13:latest
FROM ${BASE_IMAGE}
LABEL org.rptadvanced.test.project="rptadv-hamlib-adapter"
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends libhamlib-dev && \
    pkg-config --atleast-version=4.6.2 hamlib && rm -rf /var/lib/apt/lists/*
WORKDIR /workspace
