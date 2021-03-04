FROM ubuntu:20.04 as base

USER root

WORKDIR /build

SHELL [ "/bin/bash", "-c" ]

ENV DEBIAN_FRONTEND noninteractive
RUN apt-get update && apt-get upgrade -y

RUN apt install -y gcc curl jq 

RUN curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf >rustup.rs && bash rustup.rs -y

ENV PATH="$PATH:/root/.cargo/bin"
RUN echo $PATH && ls $HOME && rustup default stable && cargo version && \
    rustup update stable

RUN rustup target add wasm32-unknown-unknown

ARG CONTRACT_MOUNT="/contract/"
VOLUME ${CONTRACT_MOUNT}
WORKDIR ${CONTRACT_MOUNT}

COPY ./scripts /scripts

ENTRYPOINT ["/scripts/compile.sh"]
CMD ["/contract"] #FIXME(LR) use ARG var