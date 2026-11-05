#!/usr/bin/env python3
"""Run radio-free Air protocol tests with synthetic credentials, on a JDK."""
from pathlib import Path
import hashlib
import os
import shutil
import subprocess
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
DEPENDENCIES = [
    ("bctls-jdk18on", "1.86.2", "8ce8d5fb1b5d323a73df1fdba920b2f97c12d047986028fe65bf64d3997e50f2"),
    ("bcprov-jdk18on", "1.86", "2af190b300cbb0b35e248ccf5f4a06b6072030aeb3da7a98ec73abe5b4cb371f"),
    ("bcutil-jdk18on", "1.86", "1c268e15f785aafb1e4670d8d6e5f856f69475e96c162dadcd1ff9b350e9139f"),
]

def tool(name):
    home = os.environ.get("JAVA_HOME")
    path = str(Path(home) / "bin" / name) if home else shutil.which(name)
    if not path or not Path(path).is_file():
        raise SystemExit("Set JAVA_HOME to a JDK, or put java and javac on PATH")
    return path

def main():
    java, javac = tool("java"), tool("javac")
    jars = ROOT / ".tools/protocol-jars"
    classes = ROOT / ".tools/protocol-tests"
    jars.mkdir(parents=True, exist_ok=True)
    classes.mkdir(parents=True, exist_ok=True)
    for artifact, version, expected in DEPENDENCIES:
        destination = jars / (artifact + ".jar")
        if not destination.exists():
            url = f"https://repo.maven.apache.org/maven2/org/bouncycastle/{artifact}/{version}/{artifact}-{version}.jar"
            urllib.request.urlretrieve(url, destination)
        if hashlib.sha256(destination.read_bytes()).hexdigest() != expected:
            raise SystemExit("Dependency checksum mismatch: " + artifact)
    base = ROOT / "crates/tauri-plugin-air/android/src"
    sources = sorted((base / "main/java/app/airspace/air/protocol").glob("*.java"))
    sources += sorted((base / "test/java/app/airspace/air/protocol").glob("*.java"))
    subprocess.run([javac, "-cp", str(jars / "*"), "-d", str(classes), *map(str, sources)], check=True)
    classpath = str(classes) + os.pathsep + str(jars / "*")
    for test in ("ProtocolTest", "GattlinkResetTest", "DtlsTest", "GattlinkDtlsTest"):
        subprocess.run([java, "-cp", classpath, "app.airspace.air.protocol." + test], check=True, timeout=90)

if __name__ == "__main__":
    main()
