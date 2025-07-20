#!/usr/bin/env python3
import sys
import os

print("Content-Type: text/html")
print()
print("<html><body>")
print("<h1>CGI Echo Test</h1>")
print(f"<p><strong>Request Method:</strong> {os.environ.get('REQUEST_METHOD', 'Unknown')}</p>")
print(f"<p><strong>Content Length:</strong> {os.environ.get('CONTENT_LENGTH', '0')}</p>")
print(f"<p><strong>Content Type:</strong> {os.environ.get('CONTENT_TYPE', 'Not set')}</p>")
print(f"<p><strong>Query String:</strong> {os.environ.get('QUERY_STRING', 'None')}</p>")
print(f"<p><strong>Transfer Encoding:</strong> {os.environ.get('HTTP_TRANSFER_ENCODING', 'Not set')}</p>")

# Read POST data from stdin
if os.environ.get('REQUEST_METHOD') == 'POST':
    try:
        content_length = int(os.environ.get('CONTENT_LENGTH', '0'))
        if content_length > 0:
            post_data = sys.stdin.read(content_length)
            print(f"<p><strong>POST Data:</strong> {post_data}</p>")
            print(f"<p><strong>POST Data Length:</strong> {len(post_data)} bytes</p>")
        else:
            # Try to read all available data for chunked requests
            post_data = sys.stdin.read()
            if post_data:
                print(f"<p><strong>POST Data (chunked):</strong> {post_data}</p>")
                print(f"<p><strong>POST Data Length:</strong> {len(post_data)} bytes</p>")
            else:
                print("<p><strong>POST Data:</strong> No data received</p>")
    except Exception as e:
        print(f"<p><strong>Error reading POST data:</strong> {e}</p>")

print("<h2>All Environment Variables:</h2>")
print("<ul>")
for key, value in sorted(os.environ.items()):
    if key.startswith(('HTTP_', 'REQUEST_', 'CONTENT_', 'QUERY_', 'SERVER_', 'GATEWAY_')):
        print(f"<li><strong>{key}:</strong> {value}</li>")
print("</ul>")

print("</body></html>")
