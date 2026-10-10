# The host OCI backend supplies task UID, limits and mounts.
FROM node:24-bookworm-slim AS node_runtime
FROM python:3.12.11-slim-bookworm
COPY --from=node_runtime /usr/local/bin/node /usr/local/bin/node
COPY --from=node_runtime /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
 && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx \
 && apt-get update \
 && apt-get install -y --no-install-recommends bash ca-certificates ffmpeg \
    fonts-noto-cjk fonts-dejavu-core fontconfig libreoffice-impress \
    libreoffice-writer libreoffice-calc poppler-utils pandoc \
 && rm -rf /var/lib/apt/lists/*
COPY requirements.txt /opt/exoharness/requirements.txt
RUN pip install --no-cache-dir -r /opt/exoharness/requirements.txt \
 && npm install -g --ignore-scripts pptxgenjs@4.0.1 docx@9.7.1 typescript@5.9.3 \
 && npm cache clean --force
ENV NODE_PATH=/usr/local/lib/node_modules \
    HOME=/tmp \
    MPLCONFIGDIR=/tmp/matplotlib \
    PYTHONDONTWRITEBYTECODE=1
WORKDIR /tmp
USER 65532:65532
ENTRYPOINT []
CMD ["python", "--version"]
