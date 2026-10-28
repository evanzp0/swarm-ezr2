import shelve
from utils import DB_PATH 

class Database:
    __shared_state = {}

    def __init__(self):
        self.__dict__ = self.__shared_state
    
    def get(self):
        if not hasattr(self, '__db'):
            self.__db = shelve.open(DB_PATH + "/tracker.db", writeback=True)
        return self.__db
    
    def close(self):
        if not hasattr(self, '__db'):
            return 0
        self.__db.close()
        del self.__db

def get_db():
    return Database().get()

def close_db():
    Database().close()

def get_peers(info_hash, peers_nr):
    # peers are stored as maps with IPs as keys and ports and event as values
    with get_db() as db:
        peers_map = db[info_hash]

    peers_list = []
    for k, v in peers_map.items():
        peers_list.append((k, *v))
        if peers_nr == 0:
            break
        peers_nr -= 1

    return peers_list 

