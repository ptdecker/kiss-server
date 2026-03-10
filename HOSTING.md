# AWS Hosting with TLS Services

This documents the process I used to host this dependency free server on AWS. I chose AWS given I already have some
familiarity with the process, but I'm sure you can apply the same process to Google Cloud Services or some other
hosting platform. By using a hosting platform, among other benefits, I can avoid implementing TLS support in this
simple server and thus avoid reinventing the wheel or adding significant size and dependencies to this basic
server implementation.

## Acquire a Domain

I have multiple domains registered at [GoDaddy](https://godaddy.com) and [Porkbun](https://porkbun.com). Porkbun
seems to have more sanity and less baloney than GoDaddy. But, obviously, use whatever domain registrar you want.
Here is a [comparission from 2024](https://www.wpbeginner.com/beginners-guide/how-to-choose-the-best-domain-registrar/).

## Obtain an SSL Certificate

You'll also need to obtain an SSL certificate to support TLS connectivity to your domain. Porkbun offers free
certificates for domains that use their name server (link). The certificates themselves are issued by
the nonprofit Certificate Authority [Let's Encrypt](https://letsencrypt.org). If you're more interested in
commercial options, you can obtain a certificate [directly from Amazon](https://aws.amazon.com/certificate-manager/).
Here is a [guide on obtaining one](https://hostadvice.com/blog/web-hosting/security/how-to-get-an-ssl-certificate/).

Personally, I elected to use AWS' certificate manager to avoid the hassle of having to refresh my certificates
every 75 days. Be sure to use the email validation strategy when you create your free public certificate. But,
to do this, you need to have email forwarding set up at your domain registrar. It isn't clear, but ChatGPT says
the email is sent to admin@yourdomain.com, administrator@yourdomain.com, hostmaster@yourdomain.com,
postmaster@yourdomain.com, and webmaster@yourdomain.com. I cannot vouch for the others, but I set up forwarding
from my domain registrar to admin@mydomain.com and it worked just fine.

## Set up your EC2 instance



