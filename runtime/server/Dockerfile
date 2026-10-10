# Compile the native release before building this runtime image.
# The launcher may mount a newer release without rebuilding this image.
FROM ubuntu:24.04

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3t64 libgcc-s1 \
 && rm -rf /var/lib/apt/lists/*

COPY --chown=65532:65532 --chmod=0555 release/bin/exoharness /opt/exoharness/bin/exoharness

USER 65532:65532
ENTRYPOINT ["/opt/exoharness/bin/exoharness"]
CMD ["--version"]
