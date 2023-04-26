#!/bin/bash

# RUN THIS ONE WITH SUDO
# This script will install the service file for the Open Accountability project

# Install dependencies
sudo apt-get install libleptonica-dev libtesseract-dev clang
sudo apt-get install tesseract-ocr-eng

SERVICE_NAME="open-accountability"

systemctl stop ${SERVICE_NAME}.service

systemctl disable ${SERVICE_NAME}.service

sudo rm /etc/systemd/system/${SERVICE_NAME}.service

sudo mv ./$SERVICE_NAME.service /etc/systemd/system/$SERVICE_NAME.service

echo ls -l $DISPLAY

# Remove the device info file if it exists (force a new login)
sudo rm ./.device

systemctl daemon-reload

# Start the service
systemctl start $SERVICE_NAME.service

# Enable the service to start on boot
systemctl enable $SERVICE_NAME