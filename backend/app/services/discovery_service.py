"""
Discovery Service

Handles auto-discovery of new servers on the local network.
Uses UDP broadcast to find agents.
"""

import socket
import json
import hmac
import hashlib
import threading
import time
import logging
from datetime import datetime
from typing import List, Dict, Optional

from app import db
from app.models.server import Server

logger = logging.getLogger(__name__)


class DiscoveryService:
    """
    Service for discovering new servers/agents on the network.
    """

    # Maximum allowed age for discovery responses (seconds)
    MAX_RESPONSE_AGE = 30

    def __init__(self, port=9000, secret_key: Optional[str] = None):
        self.port = port
        self.secret_key = secret_key
        self._discovered_agents = {}  # agent_id -> info
        self._is_scanning = False
        self._lock = threading.Lock()

    def _sign_discovery_request(self, request_data: dict, secret_key: str) -> dict:
        """Sign a discovery request with HMAC."""
        request_data['timestamp'] = int(time.time() * 1000)
        message = json.dumps(request_data, sort_keys=True)
        signature = hmac.new(secret_key.encode(), message.encode(), hashlib.sha256).hexdigest()
        request_data['signature'] = signature
        return request_data

    def start_scan(self, duration=10) -> List[Dict]:
        """
        Start a network scan for agents.
        """
        if self._is_scanning:
            return list(self._discovered_agents.values())

        self._is_scanning = True
        self._discovered_agents = {}
        
        # Start listening thread
        listen_thread = threading.Thread(target=self._listen_for_responses, daemon=True)
        listen_thread.start()
        
        # Send broadcast requests
        self._send_broadcast_request()
        
        # Wait for duration
        time.sleep(duration)
        
        self._is_scanning = False
        return list(self._discovered_agents.values())

    def _send_broadcast_request(self):
        """Send UDP broadcast request to discover agents"""
        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
            sock.settimeout(2)
            
            request_data = {
                'type': 'discovery_request',
                'timestamp': int(time.time() * 1000)
            }

            # Sign the request if a shared secret is configured
            if self.secret_key:
                request_data = self._sign_discovery_request(request_data, self.secret_key)

            message = json.dumps(request_data)

            # Broadcast to common local networks
            sock.sendto(message.encode(), ('255.255.255.255', self.port))
            sock.close()
        except Exception as e:
            logger.error(f"Error sending discovery broadcast: {e}")

    def _listen_for_responses(self):
        """Listen for UDP discovery responses from agents"""
        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            sock.bind(('', self.port + 1))  # Listen on port + 1
            sock.settimeout(1)
            
            start_time = time.time()
            while self._is_scanning:
                try:
                    data, addr = sock.recvfrom(4096)
                    try:
                        info = json.loads(data.decode())
                        if info.get('type') == 'discovery':
                            # Validate timestamp to prevent replay attacks
                            response_ts = info.get('timestamp')
                            if response_ts:
                                age_seconds = abs(time.time() * 1000 - response_ts) / 1000
                                if age_seconds > self.MAX_RESPONSE_AGE:
                                    logger.warning(f"Stale discovery response from {addr[0]} (age: {age_seconds:.0f}s), ignoring")
                                    continue

                            agent_id = info.get('agent_id')
                            if agent_id:
                                # Add IP address from sender
                                info['ip_address'] = addr[0]

                                # Check if already registered
                                server = Server.query.filter_by(agent_id=agent_id).first()
                                info['is_registered'] = server is not None
                                if server:
                                    info['server_name'] = server.name
                                    info['server_id'] = server.id

                                with self._lock:
                                    self._discovered_agents[agent_id] = info
                            else:
                                logger.warning(f"Discovery response from {addr[0]} missing agent_id")
                    except Exception as e:
                        logger.error(f"Error parsing discovery response: {e}")
                except socket.timeout:
                    continue
            sock.close()
        except Exception as e:
            logger.error(f"Error in discovery listener: {e}")

    def get_discovered_agents(self) -> List[Dict]:
        """Get currently discovered agents"""
        with self._lock:
            return list(self._discovered_agents.values())


discovery_service = DiscoveryService()
