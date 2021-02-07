FROM python:3.8

WORKDIR /source

COPY . .

RUN npm install