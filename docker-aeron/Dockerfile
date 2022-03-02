
FROM neomantra/aeron-cpp-debian:latest AS aeron-builder

RUN USER=root apt-get update && \
    apt-get -y upgrade && \
    apt-get -y install git curl g++ build-essential libssl-dev pkg-config && \
    apt-get -y install software-properties-common && \
    apt-get update 


RUN USER=root apt-get update
RUN  curl https://sh.rustup.rs -sSf | sh -s -- -y 

COPY ./id_ed25519 root/.ssh/
COPY ./id_ed25519.pub root/.ssh/
RUN chmod 600 /root/.ssh/id_ed25519 && \
    chmod 600 /root/.ssh/id_ed25519.pub

EXPOSE 3030
ENTRYPOINT [ "/usr/local/bin/aeronmd" ]

