
import subprocess
import os

vswhere = r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
if os.path.exists(vswhere):
    try:
        output = subprocess.check_output([vswhere, "-latest", "-products", "*", "-requires", "Microsoft.Component.MSBuild", "-property", "installationPath"], text=True).strip()
        print(f"VS_PATH={output}")
    except Exception as e:
        print(f"Error running vswhere: {e}")
else:
    print("vswhere not found at default location")
