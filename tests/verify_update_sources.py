"""Check source switching and persistence through an isolated native pipe."""
import argparse,json
from verify_pomodoro import Runtime

def run(args):
 app=Runtime(args.runtime,args.scratch);report={"success":False,"checks":[]}
 try:
  state=app.start();assert state["updateSource"]=="gitcode",state.get("updateSource")
  report["checks"].append("default GitCode")
  for source in ("gitee","github","gitcode","github"):
   reply=app.request("updateSource",source);assert reply["success"],reply
   assert app.request("state")["updateSource"]==source
  report["checks"].append("all three native source commands are acknowledged")
  reply=app.request("updateSource","invalid");assert not reply["success"]
  assert app.request("state")["updateSource"]=="github"
  report["checks"].append("invalid source cannot change selection")
  app.stop();state=app.start(preserve_settings=True);assert state["updateSource"]=="github"
  report["checks"].append("selection survives native restart")
  report["success"]=True
 finally:
  app.stop();args.report.parent.mkdir(parents=True,exist_ok=True);args.report.write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding="utf8")
 return report
if __name__=="__main__":
 from pathlib import Path
 p=argparse.ArgumentParser();p.add_argument("--runtime",type=Path,required=True);p.add_argument("--scratch",type=Path,required=True);p.add_argument("--report",type=Path,required=True);print(json.dumps(run(p.parse_args()),ensure_ascii=False))
