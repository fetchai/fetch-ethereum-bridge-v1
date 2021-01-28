FROM node:lts-buster

WORKDIR /source

COPY . .

RUN npm install