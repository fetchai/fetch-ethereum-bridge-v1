FROM ubuntu:18.04 as base

USER root

WORKDIR /workdir

SHELL [ "/bin/bash", "-c" ]

ENV DEBIAN_FRONTEND noninteractive
RUN apt-get update && apt-get upgrade -y


####################
### dependencies ###
####################

ARG wasmd_version="0.9.1"
ENV WASMD_VER=${wasmd_version}

ARG golang_version="1.15.7"
ENV GOLANG_VER=${golang_version}


# utils
RUN apt install -y wget make curl git jq 

# python 3.6
RUN apt install -y python3 python3-pip
ENV LC_ALL C.UTF-8
ENV LANG C.UTF-8

# golang
RUN wget https://golang.org/dl/go${GOLANG_VER}.linux-amd64.tar.gz && \
    tar -xzvf go${GOLANG_VER}.linux-amd64.tar.gz -C /usr/local && \
    mkdir /root/go
ENV PATH="${PATH}:/usr/local/go/bin:/root/go/bin"

# rust latest
RUN curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf >rustup.rs && bash rustup.rs -y

ENV PATH="$PATH:/root/.cargo/bin"
RUN echo $PATH && ls $HOME && rustup default stable && cargo version && \
    rustup update stable

# wasmd
RUN git clone https://github.com/CosmWasm/wasmd.git && cd wasmd && \
    git checkout v${WASMD_VER} && \
    make install && wasmd version


##########################
### setup cosmwasm env ###
##########################

RUN rustup target add wasm32-unknown-unknown


########################
### setup local node ###
########################

COPY ./docker /scripts_docker
RUN chmod +x /scripts_docker/*.sh

ARG wasmd_password="12345678"
ENV PASSWORD=${wasmd_password}

ARG token="ucosm"
ENV TOKEN=${token}

RUN /scripts_docker/00_setup_wasmd_local_v${WASMD_VER}.sh

EXPOSE 26657

ENTRYPOINT ["/bin/bash", "/scripts_docker/entrypoint.sh"]