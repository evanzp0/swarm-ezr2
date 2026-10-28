from webob import Request
from utils import *

import socket

import os
import database
import hashlib
import shelve
import urllib.parse
import bencode

def setup():
    dirname = os.path.dirname(DB_PATH)
    if not os.path.exists(dirname):
        os.makedirs(dirname)


class Tracker:
    def __call__(self, environ, start_response):
        print("Running", flush=True)
        req = Request(environ)

        if req.method != 'GET':
            start_response(HTTP_400_BAD_REQUEST, [CONTENT_HEADER])
            return [HTTP_400_MESSAGE]


        if req.path_info != '/announce:6969':
            start_response(HTTP_404_NOT_FOUND, [CONTENT_HEADER])
            return [HTTP_404_MESSAGE]

        params = query_to_map(environ['QUERY_STRING'])

        info_hash  = get_value(params, 'info_hash',  lambda x: urllib.parse.unquote_to_bytes(x).hex())
        peer_id    = get_value(params, 'peer_id',  None)
        port       = get_value(params, 'port',       int)
        compact    = get_value(params, 'compact',    int)
        event      = get_value(params, 'event',      None)
        numwant    = get_value(params, 'numwant',    int)

        if not info_hash:
            start_response(HTTP_200_OK, [CONTENT_HEADER])
            return self.build_fail_response(fail_reason=MISSING_INFO_HASH)
        if not peer_id:
            start_response(HTTP_200_OK, [CONTENT_HEADER])
            return self.build_fail_response(fail_reason=MISSING_PEER_ID)
        if not port:
            start_response(HTTP_200_OK, [CONTENT_HEADER])
            return self.build_fail_response(fail_reason=MISSING_PORT)
        if not compact:
            start_response(HTTP_200_OK, [CONTENT_HEADER])
            return self.build_fail_response(fail_reason=INVALID_COMPACT)
        if not event:
            start_response(HTTP_200_OK, [CONTENT_HEADER])
            return self.build_fail_response(fail_reason=MISSING_EVENT)
        if not numwant or numwant > MAX_PEERS:
            start_response(HTTP_200_OK, [CONTENT_HEADER])
            return self.build_fail_response(fail_reason=INVALID_NUMWANT)

        if not self.has_torrent(info_hash):
            start_response(HTTP_404_NOT_FOUND, [CONTENT_HEADER])
            return [HTTP_404_MESSAGE]

        ip = get_client_addr(environ)

        if event == 'stopped':
            self.remove_peer_from_db(info_hash, ip)
        else:
            self.add_peer_to_db(info_hash, ip, port, event)

        start_response(HTTP_200_OK, [CONTENT_HEADER])
        return self.build_response(info_hash, numwant, 10, ip)
    
    def has_torrent(self, info_hash: str) -> bool:
        with database.get_db() as db:
            return info_hash in db
    
    def add_peer_to_db(self, info_hash, ip, port, event):
        with database.get_db() as db:
            db[info_hash][ip] = (port, event)

    def remove_peer_from_db(self, info_hash, ip):
        with database.get_db() as db:
            if ip not in db[info_hash]:
                return
            del db[info_hash][ip]
    
    def build_fail_response(self, fail_reason: str):
        return [bencode.dumps({"failure reason": fail_reason})]    

    def build_response(
        self,
        info_hash: str, 
        numwant:   int, 
        interval:  int,
        client_ip: str,
        *,
        warn:      str = "",
    ):

        peers_list = database.get_peers(info_hash, numwant)    

        peers_compact_str = b""
        complete   = 0
        incomplete = 0
        for ip, port, event in peers_list:
            # TODO
            # don't send a client its own ip (this can be done more efficiently)
            if ip == client_ip:
                continue
            peers_compact_str += socket.inet_aton(ip) 
            peers_compact_str += port.to_bytes(2, byteorder="big")
            
            if event == "completed":
                complete += 1
            else:
                incomplete += 1

        response = {
            "interval":   interval,
            "complete":   complete,
            "incomplete": incomplete,
            "peers":      peers_compact_str 
        }

        if warn:
            response["warning message"] = message
        
        return [bencode.dumps(response)]


setup()
tracker = Tracker()
