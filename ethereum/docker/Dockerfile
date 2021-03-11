FROM python:3.8

RUN pip install pipenv

WORKDIR /source

COPY . .

ENV NETWORK=docker
#ENV DEPLOYMENT_PRIV_KEY_PATH=
#ENV DEPLOYMENT_PRIV_KEY_PWD=
ENV ETH_CONTRACT_DEPLOYMENT_MANIFEST_PATH=/config/deployment_manifest.json
#VOLUME /config

RUN pipenv install --system --deploy --ignore-pipfile
RUN pipenv run brownie networks import networks-config.yaml True
RUN pipenv run brownie compile

ENTRYPOINT pipenv run brownie run deploy_bridge.py --network ${NETWORK}
