#!/usr/bin/env python3

import argparse
import json
import os
from datetime import datetime
from pathlib import Path
import pandas as pd
import matplotlib.pyplot as plt
import seaborn as sns
from jinja2 import Template
import yaml

class BenchmarkReportGenerator:
    def __init__(self, results_dir, config_path, timestamp):
        self.results_dir = Path(results_dir)
        self.timestamp = timestamp
        self.config = self._load_config(config_path)
        self.results = self._load_results()
        
    def _load_config(self, config_path):
        with open(config_path) as f:
            return yaml.safe_load(f)
    
    def _load_results(self):
        results = []
        for file in self.results_dir.glob(f"*_{self.timestamp}.json"):
            with open(file) as f:
                results.append(json.load(f))
        return results
    
    def generate_plots(self):
        """Generate all plots specified in config"""
        plots = {}
        for plot_config in self.config['reporting']['plots']:
            if plot_config['type'] == 'bar':
                plots.update(self._generate_bar_plots(plot_config['metrics']))
            elif plot_config['type'] == 'line':
                plots.update(self._generate_line_plots(plot_config['metrics']))
        return plots
    
    def _generate_bar_plots(self, metrics):
        plots = {}
        df = pd.DataFrame(self.results)
        
        for metric in metrics:
            plt.figure(figsize=(10, 6))
            sns.barplot(data=df, x='system', y=f'summary.avg_{metric}')
            plt.title(f'Average {metric.replace("_", " ").title()} by System')
            plt.xticks(rotation=45)
            
            plot_path = self.results_dir / f'plot_{metric}_{self.timestamp}.png'
            plt.savefig(plot_path, bbox_inches='tight')
            plt.close()
            
            plots[metric] = str(plot_path)
        
        return plots
    
    def _generate_line_plots(self, metrics):
        plots = {}
        for metric in metrics:
            plt.figure(figsize=(12, 6))
            for result in self.results:
                measurements = pd.DataFrame(result['measurements'])
                plt.plot(measurements.index, measurements[metric], 
                        label=result['system'])
            
            plt.title(f'{metric.replace("_", " ").title()} Over Time')
            plt.legend()
            
            plot_path = self.results_dir / f'plot_{metric}_time_{self.timestamp}.png'
            plt.savefig(plot_path, bbox_inches='tight')
            plt.close()
            
            plots[f'{metric}_time'] = str(plot_path)
        
        return plots
    
    def generate_tables(self):
        """Generate summary tables"""
        tables = {}
        df = pd.DataFrame(self.results)
        
        for table_config in self.config['reporting']['tables']:
            if table_config['name'] == 'summary':
                tables['summary'] = self._generate_summary_table(df)
            elif table_config['name'] == 'detailed':
                tables['detailed'] = self._generate_detailed_table(df)
                
        return tables
    
    def _generate_summary_table(self, df):
        metrics = self.config['reporting']['tables'][0]['metrics']
        return df[['system'] + metrics].to_html(index=False)
    
    def _generate_detailed_table(self, df):
        return df.to_html(index=False)
    
    def generate_html_report(self):
        """Generate final HTML report"""
        plots = self.generate_plots()
        tables = self.generate_tables()
        
        template = Template('''
        <!DOCTYPE html>
        <html>
        <head>
            <title>ZK Benchmark Results - {{ timestamp }}</title>
            <style>
                body { font-family: Arial, sans-serif; margin: 40px; }
                .plot { margin: 20px 0; }
                .table { margin: 20px 0; }
                h2 { color: #333; }
            </style>
        </head>
        <body>
            <h1>ZK Benchmark Results</h1>
            <p>Generated: {{ timestamp }}</p>
            
            <h2>Summary</h2>
            <div class="table">
                {{ tables.summary }}
            </div>
            
            <h2>Performance Plots</h2>
            {% for metric, plot in plots.items() %}
            <div class="plot">
                <h3>{{ metric | replace('_', ' ') | title }}</h3>
                <img src="{{ plot }}" alt="{{ metric }} plot">
            </div>
            {% endfor %}
            
            <h2>Detailed Results</h2>
            <div class="table">
                {{ tables.detailed }}
            </div>
        </body>
        </html>
        ''')
        
        html = template.render(
            timestamp=self.timestamp,
            plots=plots,
            tables=tables
        )
        
        report_path = self.results_dir / f'report_{self.timestamp}.html'
        with open(report_path, 'w') as f:
            f.write(html)
        
        return report_path

def main():
    parser = argparse.ArgumentParser(description='Generate benchmark report')
    parser.add_argument('--results-dir', required=True, help='Directory containing benchmark results')
    parser.add_argument('--config', required=True, help='Path to configuration file')
    parser.add_argument('--timestamp', required=True, help='Timestamp of the benchmark run')
    args = parser.parse_args()
    
    generator = BenchmarkReportGenerator(args.results_dir, args.config, args.timestamp)
    report_path = generator.generate_html_report()
    print(f"Report generated: {report_path}")

if __name__ == '__main__':
    main()