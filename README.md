# whoami

```sh
docker pull umutbasal/whoami:latest

docker run -p 8080:8080 --rm -it --name wh umutbasal/whoami:latest
```

## Output

- Request Remote Address
- Request Headers
- Environment Variables
- System Information
  - CPU
  - Disk
  - Memory
  - Network
  - Hostname
  - Users
  - OS Information
  - Boot Time
  - ... (see [sysinfo](https://docs.rs/sysinfo/latest/sysinfo/) crate)
