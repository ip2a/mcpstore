
```bash
# Start documentation service
cd docker/doc && docker-compose up -d

# Start frontend service
cd docker/web && docker-compose up -d

# Start API service
cd docker/api && docker-compose up -d

# Start Wiki service
cd docker/wiki && docker-compose up -d
```


## Directory Structure

```
docker/
├── doc/                    # Documentation service
│   ├── Dockerfile
│   ├── docker-compose.yml
│   └── README.md
├── web/                    # Frontend service
│   ├── Dockerfile
│   ├── docker-compose.yml
│   └── README.md
├── api/                    # API service
│   ├── Dockerfile
│   ├── docker-compose.yml
│   ├── start_api.py
│   └── README.md
├── wiki/                   # Wiki service
│   ├── Dockerfile
│   ├── docker-compose.yml
│   ├── requirements.txt
│   └── README.md
├── start-all.sh           # Start all services
├── stop-all.sh            # Stop all services
├── status.sh              # Check status
└── README.md              # This file
```