import ftplib
import smtplib
import urllib.request

import requests
from requests import post
from serial import Serial

# Flagged: no timeout.
requests.get("https://example.com")
requests.request("GET", "https://example.com")
post("https://example.com", data={})
urllib.request.urlopen("https://example.com")
smtplib.SMTP("localhost")
ftplib.FTP("ftp.example.com")
Serial("/dev/ttyS0")

# Not flagged: timeout given.
requests.get("https://example.com", timeout=10)
smtplib.SMTP("localhost", timeout=5)
urllib.request.urlopen("https://example.com", timeout=30)

# Not flagged: not an external request method.
requests.Session()
open("https://example.com")
