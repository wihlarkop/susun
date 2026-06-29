FROM alpine:3.20 AS runtime
ARG APP_MODE=default
RUN printf '%s\n' "$APP_MODE" > /app-mode.txt
CMD ["cat", "/app-mode.txt"]
