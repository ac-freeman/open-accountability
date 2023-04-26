#!/bin/bash

HOSTNAME=$(hostname)
echo $HOSTNAME

# From https://unix.stackexchange.com/questions/429092/what-is-the-best-way-to-find-the-current-display-and-xauthority-in-non-interacti
DISPLAY=$(ps -u $(id -u) -o pid= | xargs -I{} cat /proc/{}/environ 2>/dev/null | tr '\0' '\n' | grep -m1 '^DISPLAY=' | grep -o '[^=]*$' | cut -d':' -f2)
XAUTHORITY=$(ps -u $(id -u) -o pid= | xargs -I{} cat /proc/{}/environ 2>/dev/null | tr '\0' '\n' | grep -m1 '^XAUTHORITY=' | grep -o '[^=]*$' | cut -d':' -f2)

echo "Display: ${DISPLAY}"
echo "Xauthority: ${XAUTHORITY}"

if [ -z "$DISPLAY" ]; then
    echo "Display variable is empty. Exiting."
    exit 1
fi

xauth generate ":${DISPLAY}" . trusted
echo "generated ..."

# Get magic cookie from `xauth list`
MAGIC=$(xauth list | head -n 1 | awk '{print $NF}')

echo "Magic cookie: ${MAGIC}"

xauth add ":${DISPLAY}" MIT-MAGIC-COOKIE-1 "$MAGIC"

APPDIR=$(pwd)

SERVICE_NAME="open-accountability"
SERVICE_DESC="OpenAccountability"
SERVICE_BIN="${APPDIR}/open-accountability"
SERVICE_USER="${USER}"
SERVICE_GROUP="${USER}"
SERVICE_PID_FILE="/var/run/$SERVICE_NAME.pid"
SERVICE_LOG_FILE="/var/log/$SERVICE_NAME.log"
ENVIRONMENT="DISPLAY=:${DISPLAY}"
ENVIRONMENT_FILE="/etc/systemd/system/$SERVICE_NAME.env"

echo "environment: $ENVIRONMENT"

# Define the service
echo "[Unit]
Description=$SERVICE_DESC

[Service]
Type=simple
ExecStart=$SERVICE_BIN
Restart=always
RestartSec=30s
CPUSchedulingPolicy=fifo
CPUSchedulingPriority=15
User=$SERVICE_USER
Group=$SERVICE_GROUP
WorkingDirectory=${APPDIR}
PIDFile=$SERVICE_PID_FILE
StandardOutput=journal+console
StandardError=journal+console
Environment=$ENVIRONMENT
Environment="$XAUTHORITY"

[Install]
WantedBy=multi-user.target" > ./$SERVICE_NAME.service
