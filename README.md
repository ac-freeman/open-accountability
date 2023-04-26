<p align="center"><img src="https://user-images.githubusercontent.com/19912588/234593408-75f6b162-c62e-44eb-9fea-4ed5ebe3fb72.png" data-canonical-src="https://user-images.githubusercontent.com/19912588/234593408-75f6b162-c62e-44eb-9fea-4ed5ebe3fb72.png" width="250" height="auto"/>

<h2 align="center"> Accountability software for developers, by developers.</h1>
</p>



## What is OpenAccountability?

### Overview

OpenAccountability is a project to create a simple, secure, open-source, and cross-platform pornography accountability
software. It is designed to be used by developers. It is not a replacement for professional therapy,
but it is a tool to help you on your journey to freedom.

### Motivation

Many other accountability software projects have been created, but they have the following drawbacks.
- Easily circumvented with just a little bit of technical know-how
- Expensive (relative to the server costs)
- May not work when a VPN is used, whereas many people use VPNs for privacy, security, and work
- Don't support Linux
    - Meanwhile, 40% of developers use Linux in some capacity for personal use, according to the latest [Stack Overflow
      survey](https://survey.stackoverflow.co/2022/)
- Closed-source
    - Users cannot verify that the software is not doing anything malicious, sending unnecessary data, or otherwise
      violating their privacy
    - Users cannot contribute to the project, to close workarounds for bypassing the software (and thus improve its 
      effectiveness)
- Overly complex and resource-intensive
- Overly restrictive
    - For example, some software requires you to use a specific browser, or to use a specific DNS server

### Goals

- Be simple to install and use on Linux (and eventually Windows and macOS).
- Leverage the open-source community to close workarounds for bypassing the software. 
- Any attempt to bypass the software should be detected and reported to the user's accountability partner. It should be
  practically impossible to work around the software without the user's accountability partner knowing about it.
- The software should not rely on a specific browser or network configuration, nor analyze user's network traffic.
- Do not communicate any information to the server which can pose a security risk to the user or their accountability
    partner.
  - No personally identifiable information
  - No data that can be used to track the user, their computer, network, location, browsing history, banking
    information, passwords, or keystrokes
  - No screenshots, audio recordings, or videos
- Instead, only communicate data that can be used to verify that the user is not viewing pornography.
  - This data should be as minimal and anonymous as possible, to reduce the potential impact of a security breach.
- Be free and open-source
  - Free as in "free speech." The server code may be closed source and require a subscription or other payment to keep
    the service functioning, but the client code must be open source.
      - Any fees, however, must be reasonable and transparent.
  - Leverage the open-source community to improve the software and close workarounds for bypassing it.
- The identities of the user and their accountability partner must be kept secret from the server.

### How it Works

OpenAccountability uses a simple combination of screenshots and Optical Character Recognition (OCR) to detect 
pornography. It does not analyze network traffic, nor does it rely on a specific browser or network configuration. This 
software aims to catch you in the process of seeking/searching for pornography online.

Here's the general process:
- The user installs the software on their computer. It must be granted permission to run on startup and run as a 
  background process.
- The user logs in and configures their accountability partner emails.
- At semi-random intervals, the software takes a screenshot of the user's screen. This screenshot is kept in memory
  and never saved to disk, nor is it sent to the server.
- The screenshot is split into smaller subimages, and each subimage is analyzed locally using OCR.
- Detected text is compared to a locally-stored blacklist of banned words and phrases. The software counts the number of
  banned words and phrases in each screenshot, and communicates that information to the server, along with a timestamp
  and unique identifier for the computer.
  - No other text on the user's screen is sent to the server!
- The server stores the data, and if it deems that the user is likely to be seeking/viewing pornography, sends a 
  notification to the user's accountability partner.
- If the user attempts to disable the software or its startup behavior, the software will send a notification to the
  user's accountability partner (if possible) or require a re-authentication and new setup in order to be functional
  again.


## Setup

First, create an account! You can do so at [openaccountability.web.app](https://openaccountability.web.app/). Then,
you must subscribe to the service by clicking "Access payment portal" (a free trial is available).

In these early days, the project is not yet packaged for Linux distributions. I have provided a .zip file [release](https://github.com/ac-freeman/open-accountability/releases) containing
the binaries for Linux and the install scripts. Simply download the .zip file, extract it where you want the binary 
and log files to reside, and run `./INSTALL.sh` in your terminal **WITHOUT SUDO**. You will be prompeted partway 
through the script's execution to provide sudo permission. As of now, I've only tested on Ubuntu 20.04. Modifications 
to the install script will be necessary to run on other distributions.

## Contributing

First, check the [issues]((https://github.com/ac-freeman/open-accountability/issues)) to see if someone is already
working on your problem. If not, open an issue to discuss your idea. If you want to work on an issue, please comment on
the issue to let others know that you are working on it.

If you make a significant contribution, I would be happy to give you a free subscription promo code to thank you for
your efforts!

## Pricing

The server infrastructure and email delivery platform cost money to run, so a subscription model is necessary. I
offer a 7-day free trial to all new users, however, to ensure that the software works for you before you pay for it.
One subscription allows you to use the software on as many computers as you want, but you may only have three
accountability partners (who are the same for each device).

## Reporting Issues

If you have issues with your individual account, send me a private email at 
[openaccountability@acfreeman.dev](mailto:openaccountability@acfreeman.dev). More general issues should be reported
on the [GitHub issues page](https://github.com/ac-freeman/open-accountability/issues).

## Other resources
This project does not aim to be a replacement for professional therapy. If you are struggling with pornography
addiction, please seek professional help, and (if you are a Christian) seek pastoral counseling. This 
project also (at this time) does not target mobile devices, which are a common way to access pornography. I highly
recommend [EverAccountable](https://everaccountable.com/) for Android devices. If you use iOS, Apple's restrictions
make adequate accountability software impossible, but if you're a Linux user that probably doesn't surprise you.

## Community Code of Conduct
Do not be a jerk. Don't disparage accountability efforts, Christianity, or the Bible. Don't use swear words in your
communication or commits. Don't undermine the goals of the project. You don't have to be a religious adherent to
participate, but you do have to be respectful of the project's goals.
