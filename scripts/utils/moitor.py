#!/usr/bin/env python3

import psutil
import time
import json
import argparse
from pathlib import Path
from datetime import datetime

class SystemMonitor:
    def __init__(self, pid, output_dir):
        self.pid = pid
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.process = psutil.Process(pid)
        self.start_time = datetime.now()
        
    def collect_metrics(self):
        try:
            cpu_percent = self.process.cpu_percent()
            memory_info = self.process.memory_info()
            io_counters = self.process.io_counters()
            
            return {
                "timestamp": datetime.now().isoformat(),
                "cpu_percent": cpu_percent,
                "memory_rss": memory_info.rss,
                "memory_vms": memory_info.vms,
                "io_read_bytes": io_counters.read_bytes,
                "io_write_bytes": io_counters.write_bytes,
                "num_threads": self.process.num_threads(),
                "cpu_freq": psutil.cpu_freq().current if hasattr(psutil.cpu_freq(), 'current') else None,
            }
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            return None

    def monitor(self, interval=0.1):
        output_file = self.output_dir / f"monitor_{self.pid}_{int(time.time())}.json"
        metrics = []
        
        try:
            while self.process.is_running():
                metric = self.collect_metrics()
                if metric:
                    metrics.append(metric)
                time.sleep(interval)
        except psutil.NoSuchProcess:
            pass
        finally:
            # Save collected metrics
            with output_file.open('w') as f:
                json.dump({
                    "pid": self.pid,
                    "start_time": self.start_time.isoformat(),
                    "end_time": datetime.now().isoformat(),
                    "metrics": metrics
                }, f, indent=2)
            
            return output_file

def main():
    parser = argparse.ArgumentParser(description='Monitor system resources for a process')
    parser.add_argument('pid', type=int, help='Process ID to monitor')
    parser.add_argument('--output-dir', default='/zkvm-benchmarking/logs/monitoring',
                      help='Directory to store monitoring results')
    parser.add_argument('--interval', type=float, default=0.1,
                      help='Monitoring interval in seconds')
    
    args = parser.parse_args()
    
    monitor = SystemMonitor(args.pid, args.output_dir)
    output_file = monitor.monitor(args.interval)
    print(f"Monitoring data saved to: {output_file}")

if __name__ == '__main__':
    main()