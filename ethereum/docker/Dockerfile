FROM python:3.8

RUN pip install pipenv

WORKDIR /source

COPY . .

ENV NETWORK=docker

VOLUME /source/de

RUN pipenv install --system --deploy --ignore-pipfile
RUN pipenv run scripts/