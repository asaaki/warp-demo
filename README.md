# warp-demo

Demonstration of using task local and the wrapping of warp filters as services for state data over full request-respone cycle.
Here we play with a RequestId data structure, which gets initialized conditionally and passed around, so it can be used in headers and body data.

Motivation came from this issue and the discussion: https://github.com/seanmonstar/warp/issues/134
Code is based upon https://github.com/seanmonstar/warp/blob/master/examples/rejections.rs
and this snippet https://github.com/seanmonstar/warp/pull/408#issuecomment-578157715

Provides `/math/<u16>` route and requires a `div-by: <u16>` header for doing the calculation.
Implements a "division by zero" error case, but also handles a few others.

Example:
```sh
curl -i http://localhost:3030/math/4 -H 'div-by: 2'
```

and should return a response like:
```txt
HTTP/1.1 200 OK
content-type: application/json
x-request-id: internal-87ca5d23-7d18-4485-b0c1-bff48a67a9a4
content-length: 231
date: Mon, 20 Apr 2020 14:32:29 GMT

{
  "op": "4 / 2",
  "output": 2,
  "taskLocals": {
    "RequestIdInstance": {
      "data": "87ca5d23-7d18-4485-b0c1-bff48a67a9a4",
      "scope": "Internal"
    },
    "note": "this data is injected after warp service ran",
  }
}
```

You can also attach a `-H 'x-request-id: my-external-request-id'` header and see the expected result:

```txt
HTTP/1.1 200 OK
content-type: application/json
x-request-id: my-external-request-id
content-length: 217
date: Mon, 20 Apr 2020 14:35:25 GMT

{
  "op": "4 / 2",
  "output": 2,
  "taskLocals": {
    "RequestIdInstance": {
      "data": "my-external-request-id",
      "scope": "External"
    },
    "note": "this data is injected after warp service ran",
  }
}
```

