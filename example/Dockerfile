FROM alpine:3.21
COPY --from=ghcr.io/finatext/orgu:latest /usr/local/bin/orgu /usr/local/bin/orgu

WORKDIR /usr/src/

RUN apk add --update-cache curl

WORKDIR /tmp/download

ENV VERSION=8.23.3
ENV TARGET=linux_x64
# From https://github.com/gitleaks/gitleaks/releases/
ENV SHA256_SUM=73a35edc2285afd689e712b8e0ebad3f2eaf94b0d67cd6e1f0ec693ac751bb4a
RUN curl --silent --show-error --fail --connect-timeout 3 --max-time 10 --retry 3 \
  --location --remote-name \
  "https://github.com/gitleaks/gitleaks/releases/download/v${VERSION}/gitleaks_${VERSION}_${TARGET}.tar.gz"
RUN echo "${SHA256_SUM} gitleaks_${VERSION}_${TARGET}.tar.gz" | sha256sum -c
RUN tar --extract --gzip --file "gitleaks_${VERSION}_${TARGET}.tar.gz" --verbose
RUN install gitleaks /usr/local/bin/gitleaks
# For gitleaks.
RUN apk add --update-cache git

ENV VERSION=0.1.10
ENV TARGET=x86_64-unknown-linux-musl
# From https://github.com/Finatext/gls/releases/download/v${VERSION}/gls-${TARGET}.tar.gz.sha256
ENV SHA256_SUM=c8956c121a665d0a7e48438162467ec7268a47c649ee14d81f1338f7bc866e58
RUN curl --silent --show-error --fail --connect-timeout 3 --max-time 10 --retry 3 \
  --location --remote-name \
  "https://github.com/Finatext/gls/releases/download/v${VERSION}/gls-${TARGET}.tar.gz"
RUN echo "${SHA256_SUM} gls-${TARGET}.tar.gz" | sha256sum -c
RUN tar --extract --gzip --file "gls-${TARGET}.tar.gz" --verbose
RUN install gls /usr/local/bin/gls

ENV VERSION=0.17.2
ENV TARGET=Linux_x86_64
# https://github.com/reviewdog/reviewdog/releases/download/v0.17.2/checksums.txt
ENV SHA256_SUM=1cb3fc67bd15225423f50ff06e543f7a4bb9be5c3952996848ec412a6cb4c390
RUN curl --silent --show-error --fail --connect-timeout 3 --max-time 10 --retry 3 \
  --location --remote-name \
  "https://github.com/reviewdog/reviewdog/releases/download/v${VERSION}/reviewdog_${VERSION}_${TARGET}.tar.gz"
RUN echo "${SHA256_SUM} reviewdog_${VERSION}_${TARGET}.tar.gz" | sha256sum -c
RUN tar --extract --gzip --file "reviewdog_${VERSION}_${TARGET}.tar.gz" --verbose
RUN install reviewdog /usr/local/bin/reviewdog

# Dependencies for our job
RUN apk add --update-cache bash

WORKDIR /usr/src/
RUN rm -rf /tmp/download
COPY . .

ENTRYPOINT ["/usr/local/bin/orgu", "runner", "lambda", "--"]
# current work dir will be in cloned repository, so use absolute path.
CMD ["bash", "/usr/src/job.sh"]
