FROM python:3.12

RUN pip install pipenv

WORKDIR /source

COPY . .

ENV NETWORK=docker
#ENV DEPLOYMENT_PRIV_KEY_PATH=
#ENV DEPLOYMENT_PRIV_KEY_PWD=
ENV ETH_CONTRACT_DEPLOYMENT_MANIFEST_PATH=/config/deployment_manifest.json
#VOLUME /config

RUN pipenv install --system --deploy --ignore-pipfile
RUN python -c "from pathlib import Path; import urllib.request; p = Path('/root/.solcx/solc-v0.7.6'); p.parent.mkdir(exist_ok=True); urllib.request.urlretrieve('https://raw.githubusercontent.com/ethereum/solc-bin/gh-pages/linux-amd64/solc-linux-amd64-v0.7.6+commit.7338295f', p); p.chmod(0o755)"
RUN pipenv run brownie networks import networks-config.yaml True
RUN pipenv run brownie compile

ENTRYPOINT pipenv run brownie run deploy_bridge.py --network ${NETWORK}
