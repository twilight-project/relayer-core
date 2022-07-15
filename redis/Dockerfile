FROM redis:7.0.3
# FROM redis:6.2.6

COPY config/redis.conf /etc/redis/redis.conf
RUN chown -Rv redis /etc/redis/redis.conf
# RUN mkdir /etc/redis/
RUN chown redis:redis /etc/redis/
CMD [ "redis-server", "/etc/redis/redis.conf" ]