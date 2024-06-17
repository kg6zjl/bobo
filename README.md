# Here's Bobo
Bobo loves to answer your api requests and is always ready to throw some error codes your way. Need new routes as you test? No problem!

## Usager Examples
Simple echo of anything you POST
```
$ curl -XPOST localhost:8080/echo -d 'Hello world!'
Hello world!

$ curl -XPOST localhost:8080/echo -d '{"user": "John Doe"}'
{"user": "John Doe"}
```

Simulate any number of random errors (with a mix of 200's for testing success)
```
$ parallel -N0 "curl -I -XGET -s localhost:8080/errors" ::: {1..100}
```

Return the requested status code
```
$ curl -v -s localhost:8080/status/503 2>&1 | grep HTTP
> GET /status/503 HTTP/1.1
< HTTP/1.1 503 Service Unavailable

$ curl -v -s localhost:8080/status/401 2>&1 | grep HTTP
> GET /status/401 HTTP/1.1
< HTTP/1.1 401 Unauthorized
```

Get healthz
```
$ curl -s localhost:8080/healthz
OK
```

Return hostname:
```
curl -s localhost:8080/host
Laptop.local
```

Add routes dynamically:
```
$ curl -X POST localhost:8080/routes -d '[
    {"path": "newroute", "method": "GET", "response": "woo hoo", "code": 200},
    {"path": "users/alice", "method": "GET", "response": "Alice has worked here literally forever.", "code": 200},
    {"path": "users/bob", "method": "DELETE", "response": "", "code": 202}
]'

$ curl -s -XGET localhost:8080/newroute
woo hoo

$ curl -s -XGET localhost:8080/users/alice
Alice has worked here literally forever.

$ curl -I -XDELETE localhost:8080/users/bob
HTTP/1.1 202 Accepted
content-length: 0
date: Mon, 17 Jun 2024 05:31:01 GMT
```

