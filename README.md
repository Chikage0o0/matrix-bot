# Matrix Bot

## 介绍
利用Matrix接口，实现Webhook、Yande 图片推送到Matrix的功能，支持E2EE信息推送。

## 使用
#### 命令行
```bash
./matrix_bot -S "https://ssdfsad" -u "sdfsadf" -p "1asdfasdf"
```
#### Docker
```bash
docker run -d --name matrix_bot          \
    -e HOMESERVER_URL="https://xxx.xxx" \
    -e USERNAME="x"                      \
    -e PASSWORD="x"                      \
    -v ./matrix_bot:/matrix_bot          \
    --restart unless-stopped chikage/matrix_bot:latest
```
插件的配置文件在`/matrix_bot/plugins`目录下