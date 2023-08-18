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

### Output types

- JSON
  - if `Accept: application/json` header is present
  - if `?json=true` query parameter is present
  - if user agent is `curl`
- HTML
  - default behavior when visiting from browser

### Example

```sh
curl curl http://localhost:8080/ | jq '.sysinfo.host_name'

"ubuntu"
```

<img width="1400" alt="image" src="https://github.com/umutbasal/whoami/assets/21194079/0712ee8e-c63b-464f-be32-47b2d6bce258">
