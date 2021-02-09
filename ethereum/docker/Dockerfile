FROM python:3.8

RUN pip install pipenv

WORKDIR /source

COPY . .

RUN pipenv install --system --deploy --ignore-pipfile