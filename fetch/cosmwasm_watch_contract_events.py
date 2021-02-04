from abc import ABC, abstractmethod
import argparse
import json
import subprocess
from time import sleep
from typing import Any, Callable, Dict, List, Optional, Tuple

CosmosAddr = str

DEFAULT_PULL_SIZE = 30
DEFAULT_PULL_SIZE_INCR_FACTOR = 1.5
QUERY_TIME_INTERVAL = 5

def _execute_cmd(cmd: List[str]) -> Tuple[str, bool]:
    """
    Run command as subprocess and wait for its termination
    """
    print(f"[D] running: {cmd}")
    proc = subprocess.Popen(cmd, shell=False, stdout=subprocess.PIPE)
    out, _ = proc.communicate()
    success = False
    try:
        if proc.wait() == 0:
            success = True
    except (
        subprocess.CalledProcessError,
        Exception,
    ) as e:
        print(f"_execute_cmd caught exception: {str(e)}")
    return out.decode('ascii'), success


class CosmosEventSelector(ABC):
    """
    Filter events based on query
    """
    @abstractmethod
    def query(self) -> str:
        """ """

    @abstractmethod
    def query_earliest(self) -> str:
        """ """

    @abstractmethod
    def query_all(self) -> str:
        """ """

class FilterContractOperation(CosmosEventSelector):
    """
    Filter specific operations call on a specific contract
    """

    def __init__(self, address: CosmosAddr, operation: str) -> None:
        self._addr = address
        self._op = operation
    
    def query(self) -> str:
        return f"'message.contract_address={self._addr}&message.module=wasm&message.action=execute&wasm.action={self._op}'"

    def query_earliest(self) -> str:
        return f"'message.contract_address={self._addr}&message.module=wasm&message.action=instantiate'"

    def query_all(self) -> str:
        return f"'message.contract_address={self._addr}&message.module=wasm'"

class CosmwasmInstantiateTx:
    """
    """

    def __init__(self, tx: Dict[str, Any]) -> None:
        try:
            self.height = tx["height"]
            self.hash = tx["txhash"]
            tx_events = tx["logs"][0]["events"]
            
            tx_message = dict()
            for event in tx_events:
                if event["type"] == "message":
                    tx_message = event
                    break
            if len(tx_message) == 0:
                raise KeyError("event 'message' not found")
            
            self.address = ""
            for attr in tx_message["attributes"]:
                if attr["key"] == "contract_address":
                    self.address = attr["value"]
                    break
            if len(self.address) == 0:
                raise KeyError("attribute 'contract_address' not found")

        except (
            KeyError,
            IndexError,
            Exception,
        ) as e:
            raise Exception(f"CosmwasInstantiateTx: while parsing tx: {str(e)}")

    @classmethod
    def parse_query_response(cls, response: str) -> Optional ["CosmwasmInstantiateTx"]:
        try:
            resp = json.loads(response)
            #print("[D] received: {}/{}".format(resp["count"], resp["total_count"]))
            txs = json.loads(response)["txs"]
            assert len(txs) == 1, f"Expected exactly one tx, got {len(txs)}"
            return CosmwasmInstantiateTx(txs[0])
        except (
            KeyError,
            Exception
        ) as e:
            raise Exception(f"CosmwasExecuteTx: while parsing query response: {str(e)}")
        

class CosmwasmExecuteTx:
    """
    """

    def __init__(self, tx: Dict[str, Any]) -> None:
        try:
            self.height = int(tx["height"])
            self.hash = tx["txhash"]
            tx_msg = tx["tx"]["value"]["msg"][0]["value"]
            self.address = tx_msg["contract"]
            self.sender = tx_msg["sender"]
            self.operation = list(tx_msg["msg"].keys())[0]
            self.message = list(tx_msg["msg"].values())[0]
        except (
            KeyError,
            IndexError,
            Exception,
        ) as e:
            raise Exception(f"CosmwasExecuteTx: while parsing tx: {str(e)}")
    
    def __lt__(self, other: "CosmwasmExecuteTx") -> bool:
        return self.height < other.height
    
    def __str__(self) -> str:
        return f"cosmwasm (height={self.height}, contract={self.address})"

    @classmethod
    def parse_query_response(cls, response: str) -> List["CosmwasmExecuteTx"]:
        txs : List[CosmwasmExecuteTx] = []

        try:
            resp = json.loads(response)
            print("[D] received: {}/{}".format(resp["count"], resp["total_count"]))
            for tx in json.loads(response)["txs"]:
                txs.append(CosmwasmExecuteTx(tx))
        except (
            KeyError,
            Exception
        ) as e:
            raise Exception(f"CosmwasExecuteTx: while parsing query response: {str(e)}")
        
        return txs



def _wasmcli_query_txs(events: str, limit: int) -> Tuple[str, bool]:
    """
    Execute wasmcli query command and returns output
    """

    txs : List[CosmwasmExecuteTx] = []

    cmd = ["wasmcli", "query", "txs", "--events", events, "--limit", str(limit)]
    return _execute_cmd(cmd)

    
class CosmosEventsWatcher:
    """
    Continuously pulls new events that matches a given format
    """

    def __init__(self, selector: CosmosEventSelector, pull_size: int = DEFAULT_PULL_SIZE, pull_size_incr_factor : float = DEFAULT_PULL_SIZE_INCR_FACTOR) -> None:
        self._selector = selector
        self._pull_size = pull_size
        self._pull_size_incr_factor = pull_size_incr_factor
        self._latest_height = self._pull_minimal_height()
        print(f"[I] starting height: {self.height}")
    
    def _pull_minimal_height(self) -> int:
        response, ok = _wasmcli_query_txs(self._selector.query_earliest(), limit=self._pull_size)
        assert ok, f"Error while running query for minimal height tx"
        tx = CosmwasmInstantiateTx.parse_query_response(response)
        assert tx is not None, "Expected exactly 1 oldest tx, got None"
        return int(tx.height)
        

    def _pull_since_height(self, height: int) -> List[CosmwasmExecuteTx]:
        txs_new : List[CosmwasmExecuteTx] = []
        count = self._pull_size

        done = False
        txs_size_previous = 0
        while True:
            response, ok = _wasmcli_query_txs(self._selector.query(), limit=int(count))
            assert ok, f"Error while running query for txs"
            txs = CosmwasmExecuteTx.parse_query_response(response)
            # if len(txs) == 0:
            #print(f"[D] query found {len(txs)} events")
            
            if len(txs) == txs_size_previous:
                return txs_new
            txs_size_previous = len(txs)
        
            txs.sort(reverse=True)
            for tx in txs:
                #print(f"[D] checking {tx}")
                if tx.height <= height:
                    done = True
                    break
                txs_new.insert(0, tx)
            
            if done:
                break

            count = count * self._pull_size_incr_factor
        
        return txs_new

    
    def observe(self, process_event: Callable[[CosmwasmExecuteTx], bool]) -> None:
        """
        Observe events and process each exactly once
        """
        
        while True:
            print("[D] querying for new events...")
            txs = self._pull_since_height(self.height)
            print(f"[I] found {len(txs)} new events")

            for tx in txs:
                print(f"[I] processing event {tx}")
                processed = False
                while not processed:
                    try:
                        processed = process_event(tx)
                    except KeyboardInterrupt:
                        print(f"[E] interrupted")
                        return
                    except Exception as e:
                        print(f"[E] while processing event: {str(e)}")
                        sleep(1)

                self.height = tx.height
            
            sleep(QUERY_TIME_INTERVAL)

        
    @property
    def height(self) -> int:
        """Current height bellow which all txs have been processed"""
        return self._latest_height
    
    @height.setter
    def height(self, new_height: int) -> None:
        """Update current height"""
        assert new_height >= self._latest_height, "Updating current height with lower value {}<{}".format(new_height, self._latest_height)

        self._latest_height = new_height
        print(f"Updated height to {self._latest_height}")


def parse_commandline():
    """
    Parse command line arguments
    """

    parser = argparse.ArgumentParser()
    parser.add_argument("contract_address", help="Contract to watch execution event for")
    parser.add_argument("operation", help="Target contract operation")

    return parser.parse_args()

def print_tx(tx: CosmwasmExecuteTx) -> bool:
    print(f"[I] processed successfully: {tx}")
    return True

if __name__ == "__main__":

   args = parse_commandline()

   contract_address = args.contract_address
   contract_operation = args.operation

   txExecute = FilterContractOperation(contract_address, contract_operation)
   txObserver = CosmosEventsWatcher(txExecute)

   txObserver.observe(process_event=print_tx)