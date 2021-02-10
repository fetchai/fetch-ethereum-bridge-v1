FROM fetchai/fetchd:0.5.3

USER root

WORKDIR /source

SHELL [ "/bin/bash", "-c" ]

ENV DEBIAN_FRONTEND noninteractive
RUN apt update && \
    apt install -y wget make curl git jq build-essential && \
    apt clean


# rust latest
RUN curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf >rustup.rs && bash rustup.rs -y

ENV PATH="$PATH:/root/.cargo/bin"
RUN echo $PATH && ls $HOME && rustup default stable && cargo version && \
    rustup update stable


##########################
### setup cosmwasm env ###
##########################

RUN rustup target add wasm32-unknown-unknown


COPY . .