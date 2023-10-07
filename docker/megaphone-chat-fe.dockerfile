# Deps stage - download dependencies
FROM node:20.8 as deps
WORKDIR /app
COPY demo/react-vite-demo/package*.json /app/
RUN npm ci

# Build stage - Produce angular production bundle
FROM node:20.8 as build
WORKDIR /app
COPY --from=deps /app /app
COPY demo/react-vite-demo/. /app

RUN npm run build

# Dist stage - assemble built app with nginx server (PROD)
FROM nginx:1.23 as dist
COPY --from=build /app/dist/ /bin/www
COPY docker/nginx/nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
CMD [ "nginx", "-g", "daemon off;" ]